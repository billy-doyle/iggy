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

use crate::api::ApiError;
use crate::components::{api_client, use_session};
use crate::permissions::{
    Flag, GLOBAL_FLAGS, PermissionsDraft, STREAM_FLAGS, TOPIC_FLAGS, global_grant_for,
};

/// Global, per-stream and per-topic checkboxes for a new user's permissions.
/// One stream and one topic are edited at a time, chosen from lists.
#[component]
pub fn PermissionsEditor(draft: Signal<PermissionsDraft>) -> Element {
    let session = use_session();
    let streams = use_resource(move || async move {
        let mut streams = api_client(session).streams().await?;
        streams.sort_by_key(|stream| stream.id);
        Ok::<_, ApiError>(streams)
    });
    let mut chosen_stream = use_signal(|| None::<u32>);
    let mut chosen_topic = use_signal(|| None::<u32>);
    let stream_id = use_memo(move || {
        chosen_stream().or_else(|| {
            let streams = streams.read();
            streams
                .as_ref()?
                .as_ref()
                .ok()?
                .first()
                .map(|stream| stream.id)
        })
    });
    let topics = use_resource(move || async move {
        let Some(stream_id) = stream_id() else {
            return Ok(Vec::new());
        };
        let mut topics = api_client(session).stream(stream_id).await?.topics;
        topics.sort_by_key(|topic| topic.id);
        Ok::<_, ApiError>(topics)
    });
    let topic_id = use_memo(move || {
        let topics = topics.read();
        let topics = topics.as_ref()?.as_ref().ok()?;
        chosen_topic()
            .filter(|chosen| topics.iter().any(|topic| topic.id == *chosen))
            .or_else(|| topics.first().map(|topic| topic.id))
    });

    let streams_read = streams.read();
    let stream_list = match &*streams_read {
        Some(Ok(streams)) => streams.as_slice(),
        _ => &[],
    };
    let customized: Vec<String> = draft
        .read()
        .customized_streams()
        .into_iter()
        .map(|id| {
            stream_list
                .iter()
                .find(|stream| stream.id == id)
                .map_or_else(|| format!("#{id}"), |stream| stream.name.clone())
        })
        .collect();

    rsx! {
        fieldset { class: "permissions",
            legend { "Global permissions" }
            div { class: "flag-grid",
                for flag in GLOBAL_FLAGS.iter() {
                    FlagCheckbox {
                        key: "{flag.key}",
                        flag,
                        checked: draft.read().global.contains(flag.key),
                        on_change: move |granted| draft.write().set_global(flag.key, granted),
                    }
                }
            }
        }
        fieldset { class: "permissions",
            legend { "Stream permissions" }
            if stream_list.is_empty() {
                p { class: "muted", "There are no streams to grant permissions on." }
            } else {
                select {
                    aria_label: "Stream",
                    onchange: move |event| {
                        chosen_stream.set(event.value().parse().ok());
                        chosen_topic.set(None);
                    },
                    for stream in stream_list {
                        option {
                            key: "{stream.id}",
                            value: "{stream.id}",
                            selected: stream_id() == Some(stream.id),
                            "{stream.name}"
                        }
                    }
                }
                if let Some(stream_id) = stream_id() {
                    div { class: "flag-grid",
                        for flag in STREAM_FLAGS.iter() {
                            {
                                let implied = draft.read().global.contains(global_grant_for(flag.key));
                                rsx! {
                                    FlagCheckbox {
                                        key: "{flag.key}",
                                        flag,
                                        checked: implied || draft.read().stream_grants(stream_id, flag.key),
                                        disabled: implied,
                                        on_change: move |granted| draft.write().set_stream(stream_id, flag.key, granted),
                                    }
                                }
                            }
                        }
                    }
                }
                if !customized.is_empty() {
                    p { class: "muted", "Custom permissions on: {customized.join(\", \")}" }
                }
            }
        }
        if let (Some(stream_id), Some(Ok(topics))) = (stream_id(), &*topics.read()) {
            fieldset { class: "permissions",
                legend { "Topic permissions" }
                if topics.is_empty() {
                    p { class: "muted", "This stream has no topics." }
                } else {
                    select {
                        aria_label: "Topic",
                        onchange: move |event| chosen_topic.set(event.value().parse().ok()),
                        for topic in topics {
                            option {
                                key: "{topic.id}",
                                value: "{topic.id}",
                                selected: topic_id() == Some(topic.id),
                                "{topic.name}"
                            }
                        }
                    }
                    if let Some(topic_id) = topic_id() {
                        div { class: "flag-grid",
                            for flag in TOPIC_FLAGS.iter() {
                                FlagCheckbox {
                                    key: "{flag.key}",
                                    flag,
                                    checked: draft.read().topic_grants(stream_id, topic_id, flag.key),
                                    on_change: move |granted| draft.write().set_topic(stream_id, topic_id, flag.key, granted),
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn FlagCheckbox(
    flag: &'static Flag,
    checked: bool,
    on_change: EventHandler<bool>,
    #[props(default)] disabled: bool,
) -> Element {
    rsx! {
        label { class: "checkbox",
            input {
                r#type: "checkbox",
                checked,
                disabled,
                onchange: move |event| on_change.call(event.checked()),
            }
            "{flag.label}"
        }
    }
}
