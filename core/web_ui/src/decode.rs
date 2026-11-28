// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! Turns message payloads and user headers into readable text.

use std::fmt;
use std::str::FromStr;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

use crate::format;
use crate::models::{HeaderField, UserHeaders};

/// How deep `unnest_json` goes. Each document stored as text starts a fresh
/// parser depth budget, so without this the unnested tree, and the recursion
/// that prints it, could grow far past what one parse allows.
const MAX_UNNEST_DEPTH: usize = 128;

/// How to read a payload, chosen in the message inspector. A `codec` user
/// header naming one of these preselects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decoder {
    String,
    Json,
    Xml,
}

impl Decoder {
    pub const ALL: [Self; 3] = [Self::String, Self::Json, Self::Xml];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::String => "string",
            Self::Json => "json",
            Self::Xml => "xml",
        }
    }

    /// The string decoder shows a payload that is not UTF-8 as hex, as the
    /// message list does, instead of replacing the bytes it cannot read.
    pub fn decode(self, payload: &[u8]) -> Result<String, DecodeError> {
        let text = match std::str::from_utf8(payload) {
            Ok(text) => text,
            Err(_) if self == Self::String => return Ok(format::hex(payload)),
            Err(_) => return Err(DecodeError::NotUtf8(self)),
        };
        let invalid = |reason: String| DecodeError::Invalid {
            decoder: self,
            reason,
        };
        match self {
            Self::String => Ok(text.to_owned()),
            Self::Json => {
                let value: Value =
                    serde_json::from_str(text).map_err(|error| invalid(error.to_string()))?;
                serde_json::to_string_pretty(&unnest_json(value, MAX_UNNEST_DEPTH))
                    .map_err(|error| invalid(error.to_string()))
            }
            // A DTD stays refused, because entity expansion can blow up a
            // small payload. The reason then says so.
            Self::Xml => roxmltree::Document::parse(text)
                .map(|_| text.to_owned())
                .map_err(|error| invalid(error.to_string())),
        }
    }
}

impl FromStr for Decoder {
    type Err = UnknownDecoder;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|decoder| decoder.as_str().eq_ignore_ascii_case(name.trim()))
            .ok_or_else(|| UnknownDecoder(name.to_owned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("the payload is not UTF-8 text, so it is not valid {}", .0.as_str().to_uppercase())]
    NotUtf8(Decoder),
    #[error("the payload is not valid {}: {reason}", .decoder.as_str().to_uppercase())]
    Invalid { decoder: Decoder, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("no decoder named {0}")]
pub struct UnknownDecoder(String);

impl UserHeaders {
    /// The decoder a `codec` user header asks for, if there is one.
    pub fn codec(&self) -> Option<String> {
        let Self::Entries(entries) = self else {
            return None;
        };
        entries
            .iter()
            .find(|entry| {
                entry
                    .key
                    .bytes()
                    .is_some_and(|bytes| bytes.as_slice() == b"codec")
            })
            .map(|entry| entry.value.to_string())
    }
}

impl HeaderField {
    fn bytes(&self) -> Option<Vec<u8>> {
        STANDARD.decode(&self.value).ok()
    }
}

/// One side of a user header, rendered by its kind. Numbers are
/// little-endian, as the server stores them. Raw bytes, and anything that
/// does not match its kind, stay as the server's base64.
impl fmt::Display for HeaderField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self
            .bytes()
            .and_then(|bytes| decode_header(&self.kind, &bytes))
        {
            Some(text) => formatter.write_str(&text),
            None => formatter.write_str(&self.value),
        }
    }
}

/// Parses string values that hold a JSON object or array, so a document
/// stored inside another as text is shown as a document. Other strings stay
/// strings, so `"123"` is not turned into a number.
fn unnest_json(value: Value, depth: usize) -> Value {
    let Some(inner_depth) = depth.checked_sub(1) else {
        return value;
    };
    match value {
        Value::String(text) if text.trim_start().starts_with(['{', '[']) => {
            match serde_json::from_str::<Value>(&text) {
                Ok(inner @ (Value::Object(_) | Value::Array(_))) => unnest_json(inner, inner_depth),
                _ => Value::String(text),
            }
        }
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| unnest_json(item, inner_depth))
                .collect(),
        ),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, field)| (key, unnest_json(field, inner_depth)))
                .collect(),
        ),
        other => other,
    }
}

fn decode_header(kind: &str, bytes: &[u8]) -> Option<String> {
    let text = match kind {
        "string" => String::from_utf8(bytes.to_vec()).ok()?,
        "bool" => (bytes.first()? != &0).to_string(),
        "int8" => i8::from_le_bytes(bytes.try_into().ok()?).to_string(),
        "int16" => i16::from_le_bytes(bytes.try_into().ok()?).to_string(),
        "int32" => i32::from_le_bytes(bytes.try_into().ok()?).to_string(),
        "int64" => i64::from_le_bytes(bytes.try_into().ok()?).to_string(),
        "int128" => i128::from_le_bytes(bytes.try_into().ok()?).to_string(),
        "uint8" => u8::from_le_bytes(bytes.try_into().ok()?).to_string(),
        "uint16" => u16::from_le_bytes(bytes.try_into().ok()?).to_string(),
        "uint32" => u32::from_le_bytes(bytes.try_into().ok()?).to_string(),
        "uint64" => u64::from_le_bytes(bytes.try_into().ok()?).to_string(),
        "uint128" => u128::from_le_bytes(bytes.try_into().ok()?).to_string(),
        "float32" => f32::from_le_bytes(bytes.try_into().ok()?).to_string(),
        "float64" => f64::from_le_bytes(bytes.try_into().ok()?).to_string(),
        _ => return None,
    };
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::HeaderEntry;

    fn field(kind: &str, bytes: &[u8]) -> HeaderField {
        HeaderField {
            kind: kind.to_owned(),
            value: STANDARD.encode(bytes),
        }
    }

    #[test]
    fn given_json_with_nested_document_should_unnest_it_and_keep_numeric_strings() {
        let payload = br#"{"order":"{\"id\":7,\"tags\":\"[1,2]\"}","sku":"123"}"#;
        let decoded = Decoder::Json.decode(payload).unwrap();
        let value: Value = serde_json::from_str(&decoded).unwrap();
        assert_eq!(value["order"]["id"], 7);
        assert_eq!(value["order"]["tags"][1], 2);
        assert_eq!(value["sku"], "123");
        assert!(decoded.contains('\n'), "should be pretty printed");
    }

    #[test]
    fn given_json_object_should_keep_its_key_order() {
        let decoded = Decoder::Json
            .decode(br#"{"zebra":1,"apple":2,"mango":3}"#)
            .unwrap();
        let positions: Vec<_> = ["zebra", "apple", "mango"]
            .iter()
            .map(|key| decoded.find(key).unwrap())
            .collect();
        assert!(positions.is_sorted(), "{decoded}");
    }

    #[test]
    fn given_nesting_past_the_budget_should_leave_strings_as_text() {
        let payload = serde_json::json!(["[1]"]);
        assert_eq!(unnest_json(payload.clone(), 1), payload);
        assert_eq!(unnest_json(payload, 2), serde_json::json!([[1]]));
    }

    #[test]
    fn given_invalid_payload_should_name_the_decoder_and_reason() {
        let error = Decoder::Json.decode(b"{not json").unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("the payload is not valid JSON: "),
            "{error}"
        );
        let error = Decoder::Xml.decode(b"<a><b></a>").unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("the payload is not valid XML: "),
            "{error}"
        );
    }

    #[test]
    fn given_well_formed_xml_should_return_it_unchanged() {
        let xml = r#"<?xml version="1.0"?><order id="7"><sku>A</sku></order>"#;
        assert_eq!(Decoder::Xml.decode(xml.as_bytes()).unwrap(), xml);
    }

    #[test]
    fn given_binary_payload_should_show_hex_or_refuse_to_parse() {
        let binary = [b'o', b'k', 0xff];
        assert_eq!(Decoder::String.decode(&binary).unwrap(), "6f 6b ff");
        assert_eq!(
            Decoder::Json.decode(&binary),
            Err(DecodeError::NotUtf8(Decoder::Json))
        );
    }

    #[test]
    fn given_decoder_name_in_any_case_should_parse() {
        assert_eq!(" JSON ".parse::<Decoder>(), Ok(Decoder::Json));
        assert!("protobuf".parse::<Decoder>().is_err());
    }

    #[test]
    fn given_header_kinds_should_decode_little_endian_values() {
        let cases = [
            (field("string", b"hello"), "hello"),
            (field("bool", &[1]), "true"),
            (field("int16", &(-2_i16).to_le_bytes()), "-2"),
            (
                field("uint64", &u64::MAX.to_le_bytes()),
                "18446744073709551615",
            ),
            (
                field("int128", &i128::MIN.to_le_bytes()),
                &i128::MIN.to_string(),
            ),
            (field("float64", &1.5_f64.to_le_bytes()), "1.5"),
        ];
        for (header, expected) in cases {
            assert_eq!(header.to_string(), expected, "{}", header.kind);
        }
    }

    #[test]
    fn given_raw_or_mismatched_header_should_keep_base64() {
        let raw = field("raw", &[1, 2, 3]);
        assert_eq!(raw.to_string(), raw.value);
        let short = field("uint32", &[1, 2]);
        assert_eq!(short.to_string(), short.value);
    }

    #[test]
    fn given_codec_header_should_find_its_value() {
        let trace = HeaderEntry {
            key: field("string", b"trace"),
            value: field("uint8", &[1]),
        };
        let codec = HeaderEntry {
            key: field("string", b"codec"),
            value: field("string", b"json"),
        };
        let headers = UserHeaders::Entries(vec![trace.clone(), codec]);
        assert_eq!(headers.codec().as_deref(), Some("json"));
        assert_eq!(UserHeaders::Entries(vec![trace]).codec(), None);
        assert_eq!(UserHeaders::Raw("AAEC".to_owned()).codec(), None);
    }
}
