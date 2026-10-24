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
use crate::components::{ErrorView, Loading, use_api};
use crate::models::Topic;
use crate::permissions::{Flag, GLOBAL_FLAGS, PermissionsDraft, STREAM_FLAGS, TOPIC_FLAGS};

/// The topics of one stream, tagged with that stream's ID.
type StreamTopics = (u32, Result<Vec<Topic>, ApiError>);

/// Global, per-stream and per-topic checkboxes for a new user's permissions.
/// One stream and one topic are edited at a time, chosen from lists.
#[component]
pub fn PermissionsEditor(draft: Signal<PermissionsDraft>) -> Element {
    let api = use_api();
    let streams = use_resource(move || async move {
        let mut streams = api.client().streams().await?;
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
        let stream_id = stream_id()?;
        let topics = api.client().stream(stream_id).await.map(|stream| {
            let mut topics = stream.topics;
            topics.sort_by_key(|topic| topic.id);
            topics
        });
        Some::<StreamTopics>((stream_id, topics))
    });
    let topic_id = use_memo(move || {
        let topics = topics.read();
        let topics = topics_of(&topics, stream_id())?.as_ref().ok()?;
        chosen_topic()
            .filter(|chosen| topics.iter().any(|topic| topic.id == *chosen))
            .or_else(|| topics.first().map(|topic| topic.id))
    });

    let streams_read = streams.read();
    // Global permissions do not need the stream list, so only the stream
    // section waits for it or shows its error.
    let (stream_list, stream_status) = match &*streams_read {
        None => (&[][..], Some(rsx! { Loading {} })),
        Some(Err(error)) => (&[][..], Some(rsx! { ErrorView { error: error.clone() } })),
        Some(Ok(streams)) => (streams.as_slice(), None),
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
    let topics_read = topics.read();

    rsx! {
        fieldset { class: "permissions",
            legend { "Global permissions" }
            div { class: "flag-grid",
                for flag in GLOBAL_FLAGS.iter() {
                    FlagCheckbox {
                        key: "{flag.key}",
                        flag,
                        checked: draft.read().global_grants(flag.key),
                        on_change: move |granted| draft.write().set_global(flag.key, granted),
                    }
                }
            }
        }
        fieldset { class: "permissions",
            legend { "Stream permissions" }
            if let Some(status) = stream_status {
                {status}
            } else if stream_list.is_empty() {
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
                                let implied = draft.read().implies(flag, stream_id);
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
        if let Some(stream_id) = stream_id() {
            fieldset { class: "permissions",
                legend { "Topic permissions" }
                match topics_of(&topics_read, Some(stream_id)) {
                    None => rsx! { Loading {} },
                    Some(Err(error)) => rsx! { ErrorView { error: error.clone() } },
                    Some(Ok(topics)) if topics.is_empty() => rsx! {
                        p { class: "muted", "This stream has no topics." }
                    },
                    Some(Ok(topics)) => rsx! {
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
                                    {
                                        let implied = draft.read().implies(flag, stream_id);
                                        rsx! {
                                            FlagCheckbox {
                                                key: "{flag.key}",
                                                flag,
                                                checked: implied || draft.read().topic_grants(stream_id, topic_id, flag.key),
                                                disabled: implied,
                                                on_change: move |granted| draft.write().set_topic(stream_id, topic_id, flag.key, granted),
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
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

/// The loaded topics when they belong to `stream_id`. A resource keeps its
/// last value while it reloads, so after a stream change the old stream's
/// topics stay until the new ones arrive, and a click on them would grant on
/// the wrong topic.
fn topics_of(
    loaded: &Option<Option<StreamTopics>>,
    stream_id: Option<u32>,
) -> Option<&Result<Vec<Topic>, ApiError>> {
    match loaded {
        Some(Some((owner, topics))) if Some(*owner) == stream_id => Some(topics),
        _ => None,
    }
}
