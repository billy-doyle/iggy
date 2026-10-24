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

use crate::models::HeaderEntry;

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

    pub fn decode(self, payload: &[u8]) -> Result<String, DecodeError> {
        let text = String::from_utf8_lossy(payload);
        match self {
            Self::String => Ok(text.into_owned()),
            Self::Json => {
                let value: Value = serde_json::from_str(&text).map_err(|_| DecodeError(self))?;
                serde_json::to_string_pretty(&unnest_json(value)).map_err(|_| DecodeError(self))
            }
            Self::Xml => match roxmltree::Document::parse(&text) {
                Ok(_) => Ok(text.into_owned()),
                Err(_) => Err(DecodeError(self)),
            },
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
#[error("the payload is not valid {}", .0.as_str().to_uppercase())]
pub struct DecodeError(Decoder);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("no decoder named {0}")]
pub struct UnknownDecoder(String);

/// The decoder a `codec` user header asks for, if there is one.
pub fn codec(headers: &[HeaderEntry]) -> Option<String> {
    headers
        .iter()
        .find(|entry| {
            entry.key.kind == "string" && header_value(&entry.key.kind, &entry.key.value) == "codec"
        })
        .map(|entry| header_value(&entry.value.kind, &entry.value.value))
}

/// Renders one side of a user header by its kind. Numbers are little-endian,
/// as the server stores them. Raw bytes, and anything that does not match
/// its kind, stay as the server's base64.
pub fn header_value(kind: &str, base64_value: &str) -> String {
    let Ok(bytes) = STANDARD.decode(base64_value) else {
        return base64_value.to_owned();
    };
    decode_header(kind, &bytes).unwrap_or_else(|| base64_value.to_owned())
}

/// A checksum in hex, the form other Iggy tools print it in.
pub struct Checksum(pub u64);

impl fmt::Display for Checksum {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "0x{:016X}", self.0)
    }
}

/// Parses string values that hold a JSON object or array, so a document
/// stored inside another as text is shown as a document. Other strings stay
/// strings, so `"123"` is not turned into a number.
fn unnest_json(value: Value) -> Value {
    match value {
        Value::String(text) => match serde_json::from_str::<Value>(&text) {
            Ok(inner @ (Value::Object(_) | Value::Array(_))) => unnest_json(inner),
            _ => Value::String(text),
        },
        Value::Array(items) => Value::Array(items.into_iter().map(unnest_json).collect()),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, field)| (key, unnest_json(field)))
                .collect(),
        ),
        scalar => scalar,
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
    use crate::models::HeaderField;

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
    fn given_invalid_payload_should_name_the_decoder() {
        let error = Decoder::Json.decode(b"{not json").unwrap_err();
        assert_eq!(error.to_string(), "the payload is not valid JSON");
        let error = Decoder::Xml.decode(b"<a><b></a>").unwrap_err();
        assert_eq!(error.to_string(), "the payload is not valid XML");
    }

    #[test]
    fn given_well_formed_xml_should_return_it_unchanged() {
        let xml = r#"<?xml version="1.0"?><order id="7"><sku>A</sku></order>"#;
        assert_eq!(Decoder::Xml.decode(xml.as_bytes()).unwrap(), xml);
    }

    #[test]
    fn given_binary_payload_string_decoder_should_replace_invalid_bytes() {
        assert_eq!(
            Decoder::String.decode(&[b'o', b'k', 0xff]).unwrap(),
            "ok\u{fffd}"
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
            assert_eq!(
                header_value(&header.kind, &header.value),
                expected,
                "{}",
                header.kind
            );
        }
    }

    #[test]
    fn given_raw_or_mismatched_header_should_keep_base64() {
        let raw = field("raw", &[1, 2, 3]);
        assert_eq!(header_value(&raw.kind, &raw.value), raw.value);
        let short = field("uint32", &[1, 2]);
        assert_eq!(header_value(&short.kind, &short.value), short.value);
    }

    #[test]
    fn given_codec_header_should_find_its_value() {
        let headers = vec![
            HeaderEntry {
                key: field("string", b"trace"),
                value: field("uint8", &[1]),
            },
            HeaderEntry {
                key: field("string", b"codec"),
                value: field("string", b"json"),
            },
        ];
        assert_eq!(codec(&headers).as_deref(), Some("json"));
        assert_eq!(codec(&headers[..1]), None);
    }

    #[test]
    fn given_checksum_should_render_padded_hex() {
        assert_eq!(Checksum(0xBEEF).to_string(), "0x000000000000BEEF");
    }
}
