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

use dioxus::prelude::*;

use crate::components::PageHeader;
use crate::format;
use crate::settings::{
    MAX_REFRESH_INTERVAL_MS, MIN_REFRESH_INTERVAL_MS, Theme, UiSettings, parse_refresh_interval,
};

/// Preferences for this browser. The theme applies as soon as it is picked,
/// and the refresh interval when it is saved.
#[component]
pub fn WebUiSettings() -> Element {
    let mut settings = use_context::<Signal<UiSettings>>();
    let saved_interval = settings.read().refresh_interval_ms;
    let mut interval = use_signal(move || saved_interval.to_string());
    let parsed = parse_refresh_interval(&interval());
    let unchanged = parsed == Ok(saved_interval);
    let invalid = parsed.is_err();

    let mut update = move |change: &dyn Fn(&mut UiSettings)| {
        let mut next = *settings.peek();
        change(&mut next);
        next.persist();
        settings.set(next);
    };

    rsx! {
        PageHeader { title: "Web UI settings" }
        section { class: "settings-section",
            h2 { "Theme" }
            div { class: "segmented", role: "radiogroup", aria_label: "Theme",
                for theme in Theme::ALL {
                    button {
                        key: "{theme.as_str()}",
                        role: "radio",
                        aria_checked: settings.read().theme == theme,
                        class: if settings.read().theme == theme { "selected" },
                        onclick: move |_| update(&|next| next.theme = theme),
                        "{theme.label()}"
                    }
                }
            }
        }
        section { class: "settings-section",
            h2 { "Refresh interval" }
            p { class: "muted",
                "Every page reloads its data at this interval. From {MIN_REFRESH_INTERVAL_MS} ms to 1 hour."
            }
            form {
                class: "inline-form",
                onsubmit: move |event: FormEvent| {
                    event.prevent_default();
                    if let Ok(ms) = parse_refresh_interval(&interval()) {
                        update(&|next| next.refresh_interval_ms = ms);
                    }
                },
                input {
                    r#type: "number",
                    min: "{MIN_REFRESH_INTERVAL_MS}",
                    max: "{MAX_REFRESH_INTERVAL_MS}",
                    aria_label: "Refresh interval in milliseconds",
                    value: interval,
                    oninput: move |event| interval.set(event.value()),
                }
                span { class: "muted", "ms" }
                if let Ok(ms) = &parsed {
                    if *ms >= 1_000 {
                        span { class: "muted", "({format::elapsed(u64::from(*ms) * 1_000)})" }
                    }
                }
                button { r#type: "submit", disabled: invalid || unchanged, "Save" }
            }
            if let Err(message) = parsed {
                p { class: "form-error", "{message}" }
            }
        }
    }
}
