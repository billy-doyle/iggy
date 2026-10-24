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

//! The permissions a new user is created with, in the shape the server's
//! `Permissions` type expects.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};

/// A permission's JSON field name and the label the form shows for it.
#[derive(Debug, PartialEq)]
pub struct Flag {
    pub key: &'static str,
    pub label: &'static str,
}

pub static GLOBAL_FLAGS: [Flag; 10] = [
    Flag {
        key: "manage_servers",
        label: "Manage servers",
    },
    Flag {
        key: "read_servers",
        label: "Read servers",
    },
    Flag {
        key: "manage_users",
        label: "Manage users",
    },
    Flag {
        key: "read_users",
        label: "Read users",
    },
    Flag {
        key: "manage_streams",
        label: "Manage streams",
    },
    Flag {
        key: "read_streams",
        label: "Read streams",
    },
    Flag {
        key: "manage_topics",
        label: "Manage topics",
    },
    Flag {
        key: "read_topics",
        label: "Read topics",
    },
    Flag {
        key: "poll_messages",
        label: "Poll messages",
    },
    Flag {
        key: "send_messages",
        label: "Send messages",
    },
];

pub static STREAM_FLAGS: [Flag; 6] = [
    Flag {
        key: "manage_stream",
        label: "Manage stream",
    },
    Flag {
        key: "read_stream",
        label: "Read stream",
    },
    Flag {
        key: "manage_topics",
        label: "Manage topics",
    },
    Flag {
        key: "read_topics",
        label: "Read topics",
    },
    Flag {
        key: "poll_messages",
        label: "Poll messages",
    },
    Flag {
        key: "send_messages",
        label: "Send messages",
    },
];

pub static TOPIC_FLAGS: [Flag; 4] = [
    Flag {
        key: "manage_topic",
        label: "Manage topic",
    },
    Flag {
        key: "read_topic",
        label: "Read topic",
    },
    Flag {
        key: "poll_messages",
        label: "Poll messages",
    },
    Flag {
        key: "send_messages",
        label: "Send messages",
    },
];

/// The global permission that already grants a stream permission on every
/// stream, which makes setting it per stream pointless.
pub fn global_grant_for(stream_key: &str) -> &'static str {
    match stream_key {
        "manage_stream" => "manage_streams",
        "read_stream" => "read_streams",
        "manage_topics" => "manage_topics",
        "read_topics" => "read_topics",
        "poll_messages" => "poll_messages",
        _ => "send_messages",
    }
}

/// Granted permissions as sets of JSON field names. Anything not in a set is
/// denied.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PermissionsDraft {
    pub global: BTreeSet<&'static str>,
    pub streams: BTreeMap<u32, BTreeSet<&'static str>>,
    pub topics: BTreeMap<(u32, u32), BTreeSet<&'static str>>,
}

impl PermissionsDraft {
    pub fn set_global(&mut self, key: &'static str, granted: bool) {
        toggle(&mut self.global, key, granted);
    }

    pub fn set_stream(&mut self, stream_id: u32, key: &'static str, granted: bool) {
        toggle(self.streams.entry(stream_id).or_default(), key, granted);
    }

    pub fn set_topic(&mut self, stream_id: u32, topic_id: u32, key: &'static str, granted: bool) {
        toggle(
            self.topics.entry((stream_id, topic_id)).or_default(),
            key,
            granted,
        );
    }

    pub fn stream_grants(&self, stream_id: u32, key: &str) -> bool {
        self.streams
            .get(&stream_id)
            .is_some_and(|keys| keys.contains(key))
    }

    pub fn topic_grants(&self, stream_id: u32, topic_id: u32, key: &str) -> bool {
        self.topics
            .get(&(stream_id, topic_id))
            .is_some_and(|keys| keys.contains(key))
    }

    /// Streams with at least one stream or topic permission.
    pub fn customized_streams(&self) -> BTreeSet<u32> {
        let streams = self
            .streams
            .iter()
            .filter(|(_, keys)| !keys.is_empty())
            .map(|(stream_id, _)| *stream_id);
        let topics = self
            .topics
            .iter()
            .filter(|(_, keys)| !keys.is_empty())
            .map(|((stream_id, _), _)| *stream_id);
        streams.chain(topics).collect()
    }

    /// Every flag is written out, and streams and topics without a grant are
    /// left out, as the server's `Permissions` type expects.
    pub fn to_json(&self) -> Value {
        let mut streams = Map::new();
        for stream_id in self.customized_streams() {
            let mut stream = flags_json(&STREAM_FLAGS, self.streams.get(&stream_id));
            let topics: Map<String, Value> = self
                .topics
                .range((stream_id, 0)..=(stream_id, u32::MAX))
                .filter(|(_, keys)| !keys.is_empty())
                .map(|((_, topic_id), keys)| {
                    (
                        topic_id.to_string(),
                        Value::Object(flags_json(&TOPIC_FLAGS, Some(keys))),
                    )
                })
                .collect();
            if !topics.is_empty() {
                stream.insert("topics".to_owned(), Value::Object(topics));
            }
            streams.insert(stream_id.to_string(), Value::Object(stream));
        }
        json!({
            "global": flags_json(&GLOBAL_FLAGS, Some(&self.global)),
            "streams": if streams.is_empty() { Value::Null } else { Value::Object(streams) },
        })
    }
}

fn toggle(keys: &mut BTreeSet<&'static str>, key: &'static str, granted: bool) {
    if granted {
        keys.insert(key);
    } else {
        keys.remove(key);
    }
}

fn flags_json(flags: &[Flag], granted: Option<&BTreeSet<&'static str>>) -> Map<String, Value> {
    flags
        .iter()
        .map(|flag| {
            let on = granted.is_some_and(|keys| keys.contains(flag.key));
            (flag.key.to_owned(), Value::Bool(on))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_no_grants_should_deny_everything_and_omit_streams() {
        let permissions = PermissionsDraft::default().to_json();
        assert_eq!(permissions["streams"], Value::Null);
        assert_eq!(permissions["global"].as_object().unwrap().len(), 10);
        assert!(
            permissions["global"]
                .as_object()
                .unwrap()
                .values()
                .all(|granted| granted == false)
        );
    }

    #[test]
    fn given_topic_grant_only_should_write_its_stream_with_all_flags_denied() {
        let mut draft = PermissionsDraft::default();
        draft.set_global("read_streams", true);
        draft.set_topic(2, 7, "poll_messages", true);

        let permissions = draft.to_json();

        assert_eq!(permissions["global"]["read_streams"], true);
        assert_eq!(permissions["streams"]["2"]["read_stream"], false);
        assert_eq!(
            permissions["streams"]["2"]["topics"]["7"]["poll_messages"],
            true
        );
        assert_eq!(
            permissions["streams"]["2"]["topics"]["7"]["send_messages"],
            false
        );
    }

    #[test]
    fn given_revoked_grants_should_drop_empty_streams_and_topics() {
        let mut draft = PermissionsDraft::default();
        draft.set_stream(1, "send_messages", true);
        draft.set_topic(1, 3, "read_topic", true);
        draft.set_stream(1, "send_messages", false);
        draft.set_topic(1, 3, "read_topic", false);

        assert!(draft.customized_streams().is_empty());
        assert_eq!(draft.to_json()["streams"], Value::Null);
    }

    #[test]
    fn given_topics_on_neighbouring_streams_should_keep_them_apart() {
        let mut draft = PermissionsDraft::default();
        draft.set_topic(1, u32::MAX, "read_topic", true);
        draft.set_topic(2, 0, "manage_topic", true);

        let permissions = draft.to_json();

        let stream_one_topics = permissions["streams"]["1"]["topics"].as_object().unwrap();
        assert_eq!(stream_one_topics.len(), 1);
        assert!(stream_one_topics.contains_key(&u32::MAX.to_string()));
        assert_eq!(
            permissions["streams"]["2"]["topics"]["0"]["manage_topic"],
            true
        );
    }

    #[test]
    fn given_each_stream_flag_should_map_to_its_global_grant() {
        let pairs: Vec<_> = STREAM_FLAGS
            .iter()
            .map(|flag| (flag.key, global_grant_for(flag.key)))
            .collect();
        assert_eq!(
            pairs,
            [
                ("manage_stream", "manage_streams"),
                ("read_stream", "read_streams"),
                ("manage_topics", "manage_topics"),
                ("read_topics", "read_topics"),
                ("poll_messages", "poll_messages"),
                ("send_messages", "send_messages"),
            ]
        );
    }
}
