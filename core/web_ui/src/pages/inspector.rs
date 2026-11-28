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
use crate::decode::Decoder;
use crate::format;
use crate::models::{IggyMessage, UserHeaders};

/// Decoded text past this many bytes waits for "Show all", because the
/// browser lays out a whole `pre` at once and a payload can be 64 MB.
const SHOWN_TEXT_BYTES: usize = 1 << 20;

/// A message's header fields, user headers and payload, with the payload
/// read by a chosen decoder. The messages page keys it by offset, so each
/// message gets a fresh inspector and its own `codec` header.
#[component]
pub fn MessageInspector(message: Rc<IggyMessage>, on_close: EventHandler<()>) -> Element {
    let requested_codec = message.user_headers.codec();
    let codec_decoder = requested_codec
        .as_deref()
        .and_then(|codec| codec.parse::<Decoder>().ok());
    let mut decoder = use_signal(move || codec_decoder.unwrap_or(Decoder::String));
    let mut show_all = use_signal(|| false);
    let mut show_raw = use_signal(|| false);
    // Decoded once per message and once per decoder choice, not per render.
    let payload = use_hook({
        let message = Rc::clone(&message);
        move || Rc::new(STANDARD.decode(&message.payload))
    });
    let decoded = use_memo({
        let payload = Rc::clone(&payload);
        move || {
            let decoder = decoder();
            payload
                .as_ref()
                .as_ref()
                .ok()
                .map(|bytes| decoder.decode(bytes))
        }
    });

    let header = &message.header;
    let payload_view = match (payload.as_ref(), &*decoded.read()) {
        (Err(error), _) => rsx! {
            div { class: "error-box", "The payload is not valid base64: {error}" }
        },
        (Ok(bytes), _) if bytes.is_empty() => rsx! {
            p { class: "muted", "This message has no payload." }
        },
        (_, Some(Ok(text))) if text.len() > SHOWN_TEXT_BYTES && !show_all() => {
            let shown = &text[..text.floor_char_boundary(SHOWN_TEXT_BYTES)];
            rsx! {
                pre { class: "payload-text", "{shown}" }
                p { class: "muted",
                    "Showing the first {shown.len()} of {text.len()} bytes. "
                    button {
                        class: "secondary small",
                        onclick: move |_| show_all.set(true),
                        "Show all"
                    }
                }
            }
        }
        (_, Some(Ok(text))) => rsx! { pre { class: "payload-text", "{text}" } },
        (_, Some(Err(error))) => rsx! { div { class: "error-box", "{error}" } },
        (Ok(_), None) => rsx! {},
    };

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
                    dd { class: "mono", "0x{header.checksum:016X} ({header.checksum})" }
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
                                        span { class: "mono", "{entry.key}" }
                                        span { class: "muted", " → " }
                                        span { class: "mono", "{entry.value}" }
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
                                    show_all.set(false);
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
                    {payload_view}
                    // Rendered only on request, because it is a second full
                    // copy of the payload.
                    button {
                        class: "secondary small",
                        onclick: move |_| show_raw.toggle(),
                        if show_raw() { "Hide raw payload" } else { "Show raw payload (base64)" }
                    }
                    if show_raw() {
                        pre { "{message.payload}" }
                    }
                }
            }
        }
    }
}
