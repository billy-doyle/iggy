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

/// A permission's JSON field name, the label the form shows for it, and the
/// grants that already give it. The implications follow the server's
/// permissioner rules (`core/metadata/src/permissioner`).
#[derive(Debug, PartialEq)]
pub struct Flag {
    pub key: &'static str,
    pub label: &'static str,
    /// Global grants that give this permission on every stream.
    implied_by_global: &'static [&'static str],
    /// Grants on the same stream that give this permission.
    implied_by_stream: &'static [&'static str],
}

impl Flag {
    const fn new(key: &'static str, label: &'static str) -> Self {
        Self {
            key,
            label,
            implied_by_global: &[],
            implied_by_stream: &[],
        }
    }

    const fn implied_by(
        self,
        global: &'static [&'static str],
        stream: &'static [&'static str],
    ) -> Self {
        Self {
            implied_by_global: global,
            implied_by_stream: stream,
            ..self
        }
    }
}

const GLOBAL_READS: &[&str] = &[
    "manage_streams",
    "read_streams",
    "manage_topics",
    "read_topics",
];
const GLOBAL_MANAGES: &[&str] = &["manage_streams", "manage_topics"];
const STREAM_READS: &[&str] = &[
    "manage_stream",
    "read_stream",
    "manage_topics",
    "read_topics",
];
const STREAM_MANAGES: &[&str] = &["manage_stream", "manage_topics"];

pub static GLOBAL_FLAGS: [Flag; 10] = [
    Flag::new("manage_servers", "Manage servers"),
    Flag::new("read_servers", "Read servers"),
    Flag::new("manage_users", "Manage users"),
    Flag::new("read_users", "Read users"),
    Flag::new("manage_streams", "Manage streams"),
    Flag::new("read_streams", "Read streams"),
    Flag::new("manage_topics", "Manage topics"),
    Flag::new("read_topics", "Read topics"),
    Flag::new("poll_messages", "Poll messages"),
    Flag::new("send_messages", "Send messages"),
];

pub static STREAM_FLAGS: [Flag; 6] = [
    Flag::new("manage_stream", "Manage stream").implied_by(&["manage_streams"], &[]),
    Flag::new("read_stream", "Read stream")
        .implied_by(&["manage_streams", "read_streams"], &["manage_stream"]),
    Flag::new("manage_topics", "Manage topics").implied_by(GLOBAL_MANAGES, &["manage_stream"]),
    Flag::new("read_topics", "Read topics").implied_by(
        GLOBAL_READS,
        &["manage_stream", "read_stream", "manage_topics"],
    ),
    Flag::new("poll_messages", "Poll messages").implied_by(
        &[
            "manage_streams",
            "read_streams",
            "manage_topics",
            "read_topics",
            "poll_messages",
        ],
        STREAM_READS,
    ),
    Flag::new("send_messages", "Send messages").implied_by(
        &["manage_streams", "manage_topics", "send_messages"],
        STREAM_MANAGES,
    ),
];

pub static TOPIC_FLAGS: [Flag; 4] = [
    Flag::new("manage_topic", "Manage topic").implied_by(GLOBAL_MANAGES, STREAM_MANAGES),
    Flag::new("read_topic", "Read topic").implied_by(GLOBAL_READS, STREAM_READS),
    Flag::new("poll_messages", "Poll messages").implied_by(
        &[
            "manage_streams",
            "read_streams",
            "manage_topics",
            "read_topics",
            "poll_messages",
        ],
        &[
            "manage_stream",
            "read_stream",
            "manage_topics",
            "read_topics",
            "poll_messages",
        ],
    ),
    Flag::new("send_messages", "Send messages").implied_by(
        &["manage_streams", "manage_topics", "send_messages"],
        &["manage_stream", "manage_topics", "send_messages"],
    ),
];

/// Granted permissions as sets of JSON field names. Anything not in a set is
/// denied.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PermissionsDraft {
    global: BTreeSet<&'static str>,
    streams: BTreeMap<u32, BTreeSet<&'static str>>,
    topics: BTreeMap<(u32, u32), BTreeSet<&'static str>>,
}

impl PermissionsDraft {
    pub fn global_grants(&self, key: &str) -> bool {
        self.global.contains(key)
    }

    /// Whether a broader grant already gives `flag` on `stream_id`, so the
    /// form shows it as granted and locked.
    pub fn implies(&self, flag: &Flag, stream_id: u32) -> bool {
        flag.implied_by_global
            .iter()
            .any(|key| self.global.contains(key))
            || flag
                .implied_by_stream
                .iter()
                .any(|key| self.stream_grants(stream_id, key))
    }

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
    fn given_broader_grants_should_imply_narrower_flags() {
        let topic_flag = |key| TOPIC_FLAGS.iter().find(|flag| flag.key == key).unwrap();
        let stream_flag = |key| STREAM_FLAGS.iter().find(|flag| flag.key == key).unwrap();

        let mut draft = PermissionsDraft::default();
        draft.set_stream(1, "read_stream", true);
        assert!(draft.implies(topic_flag("poll_messages"), 1));
        assert!(draft.implies(topic_flag("read_topic"), 1));
        assert!(!draft.implies(topic_flag("send_messages"), 1));
        assert!(!draft.implies(topic_flag("poll_messages"), 2));

        draft.set_global("manage_topics", true);
        assert!(draft.implies(topic_flag("send_messages"), 2));
        assert!(draft.implies(stream_flag("read_topics"), 2));
        assert!(!draft.implies(stream_flag("manage_stream"), 2));
    }

    #[test]
    fn given_any_flag_should_only_be_implied_by_real_keys() {
        let global: Vec<_> = GLOBAL_FLAGS.iter().map(|flag| flag.key).collect();
        let stream: Vec<_> = STREAM_FLAGS.iter().map(|flag| flag.key).collect();
        for flag in STREAM_FLAGS.iter().chain(&TOPIC_FLAGS) {
            assert!(
                flag.implied_by_global
                    .iter()
                    .all(|key| global.contains(key))
            );
            assert!(
                flag.implied_by_stream
                    .iter()
                    .all(|key| stream.contains(key))
            );
        }
    }
}
