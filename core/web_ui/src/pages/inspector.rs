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

use std::rc::Rc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use dioxus::prelude::*;

use crate::components::Modal;
use crate::decode::{self, Checksum, Decoder};
use crate::format;
use crate::models::{IggyMessage, UserHeaders};

/// A message's header fields, user headers and payload, with the payload
/// read by a chosen decoder.
#[component]
pub fn MessageInspector(message: Rc<IggyMessage>, on_close: EventHandler<()>) -> Element {
    let requested_codec = match &message.user_headers {
        UserHeaders::Entries(entries) => decode::codec(entries),
        UserHeaders::Raw(_) => None,
    };
    let codec_decoder = requested_codec
        .as_deref()
        .and_then(|codec| codec.parse::<Decoder>().ok());
    let mut decoder = use_signal(move || codec_decoder.unwrap_or(Decoder::String));

    let header = &message.header;
    let payload = STANDARD.decode(&message.payload).unwrap_or_default();
    let decoded = decoder().decode(&payload);

    rsx! {
        Modal { title: "Message {header.offset}", on_close, wide: true,
            div { class: "inspector",
                dl { class: "details",
                    dt { "ID" }
                    dd { class: "mono", {format::message_id(header.id)} }
                    dt { "Offset" }
                    dd { "{header.offset}" }
                    dt { "Timestamp" }
                    dd { {format::timestamp_precise(header.timestamp)} }
                    dt { "Origin timestamp" }
                    dd { {format::timestamp_precise(header.origin_timestamp)} }
                    dt { "Checksum" }
                    dd { class: "mono", "{Checksum(header.checksum)}" }
                    dt { "Payload size" }
                    dd { "{header.payload_length} bytes" }
                }
                section {
                    h3 { "Headers" }
                    match &message.user_headers {
                        UserHeaders::Entries(entries) if entries.is_empty() => rsx! {
                            p { class: "muted", "No headers" }
                        },
                        UserHeaders::Entries(entries) => rsx! {
                            ul { class: "headers",
                                for (index, entry) in entries.iter().enumerate() {
                                    li { key: "{index}",
                                        span { class: "mono", {decode::header_value(&entry.key.kind, &entry.key.value)} }
                                        span { class: "muted", " → " }
                                        span { class: "mono", {decode::header_value(&entry.value.kind, &entry.value.value)} }
                                        span { class: "muted kinds", "{entry.key.kind} → {entry.value.kind}" }
                                    }
                                }
                            }
                        },
                        UserHeaders::Raw(raw) => rsx! {
                            p { class: "muted", "The server could not parse these headers. Raw bytes in base64:" }
                            pre { "{raw}" }
                        },
                    }
                }
                section { class: "payload",
                    div { class: "payload-toolbar",
                        h3 { "Payload" }
                        select {
                            aria_label: "Decoder",
                            onchange: move |event| {
                                if let Ok(next) = event.value().parse() {
                                    decoder.set(next);
                                }
                            },
                            for option_decoder in Decoder::ALL {
                                option {
                                    value: option_decoder.as_str(),
                                    selected: option_decoder == decoder(),
                                    {option_decoder.as_str().to_uppercase()}
                                }
                            }
                        }
                    }
                    if let (Some(codec), None) = (&requested_codec, codec_decoder) {
                        p { class: "muted",
                            "The codec header asks for {codec}, which the UI cannot decode. Pick a decoder instead."
                        }
                    }
                    if payload.is_empty() {
                        p { class: "muted", "This message has no payload." }
                    } else {
                        match decoded {
                            Ok(text) => rsx! { pre { class: "payload-text", "{text}" } },
                            Err(error) => rsx! { div { class: "error-box", "{error}" } },
                        }
                    }
                    details {
                        summary { "Raw payload (base64)" }
                        pre { "{message.payload}" }
                    }
                }
            }
        }
    }
}
