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

//! Mirrors of the server's HTTP JSON shapes.
//!
//! `iggy_common` owns the real types but pulls tokio with `net`, which does
//! not build for `wasm32-unknown-unknown`, so the UI keeps its own copies of
//! the fields it renders, under the server's type names. Unknown fields are
//! ignored on purpose. The tests below decode the server's serialized types
//! into each mirror, so a renamed field fails on the host.

use std::collections::BTreeMap;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct LoginUser {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IdentityInfo {
    pub user_id: u32,
    pub access_token: TokenInfo,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenInfo {
    pub token: String,
    pub expiry: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct UserInfo {
    pub id: u32,
    pub created_at: u64,
    pub status: UserStatus,
    pub username: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Inactive,
}

impl UserStatus {
    pub const ALL: [Self; 2] = [Self::Active, Self::Inactive];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

impl FromStr for UserStatus {
    type Err = UnknownUserStatus;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == text)
            .ok_or_else(|| UnknownUserStatus(text.to_owned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown user status {0}")]
pub struct UnknownUserStatus(String);

#[derive(Debug, Clone, Serialize)]
pub struct CreateUser {
    pub username: String,
    pub password: String,
    pub status: UserStatus,
    /// Built by [`crate::permissions::PermissionsDraft::to_json`].
    pub permissions: serde_json::Value,
}

/// A sparse `PUT /users/{id}` body, which the server reads as
/// `iggy_common::OptionsUpdate`.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct UserUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<UserStatus>,
}

/// The server's built-in root user always has this ID.
pub const ROOT_USER_ID: u32 = 0;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct UserInfoDetails {
    pub id: u32,
    pub username: String,
    #[serde(default)]
    pub permissions: Option<Permissions>,
}

impl UserInfoDetails {
    /// Root and holders of the global `manage_servers` permission may read
    /// the metadata change log.
    pub fn can_read_metadata_changes(&self) -> bool {
        self.id == ROOT_USER_ID
            || self
                .permissions
                .as_ref()
                .is_some_and(|permissions| permissions.global.manage_servers)
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Permissions {
    pub global: GlobalPermissions,
}

/// Only the flags the UI checks.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GlobalPermissions {
    #[serde(default)]
    pub manage_servers: bool,
}

/// `GET /stats`. Byte sizes arrive as exact-unit strings such as
/// `"8615888 KiB"`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Stats {
    pub process_id: u32,
    pub cpu_usage: f32,
    pub total_cpu_usage: f32,
    pub memory_usage: String,
    pub total_memory: String,
    pub available_memory: String,
    /// Microseconds.
    pub run_time: u64,
    pub start_time: u64,
    pub read_bytes: String,
    pub written_bytes: String,
    pub messages_size_bytes: String,
    pub streams_count: u32,
    pub topics_count: u32,
    pub partitions_count: u32,
    pub segments_count: u32,
    pub messages_count: u64,
    pub clients_count: u32,
    pub consumer_groups_count: u32,
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub iggy_server_version: String,
    pub threads_count: u32,
    pub free_disk_space: String,
    pub total_disk_space: String,
    #[serde(default)]
    pub open_files_count: u64,
    #[serde(default)]
    pub open_files_limit: u64,
}

/// One entry of a resource's `options` map, as written by
/// `iggy_common::resource_options_json`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct OptionValue {
    pub value: String,
}

pub type ResourceOptions = BTreeMap<String, OptionValue>;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Stream {
    pub id: u32,
    pub created_at: u64,
    pub name: String,
    pub size: String,
    pub messages_count: u64,
    pub topics_count: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct StreamDetails {
    pub id: u32,
    pub created_at: u64,
    pub name: String,
    pub size: String,
    pub messages_count: u64,
    pub topics_count: u32,
    pub topics: Vec<Topic>,
    #[serde(default)]
    pub options: ResourceOptions,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Topic {
    pub id: u32,
    pub created_at: u64,
    pub name: String,
    pub size: String,
    pub messages_count: u64,
    pub partitions_count: u32,
    #[serde(default)]
    pub options: ResourceOptions,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct TopicDetails {
    pub id: u32,
    pub created_at: u64,
    pub name: String,
    pub size: String,
    pub messages_count: u64,
    pub partitions_count: u32,
    pub partitions: Vec<Partition>,
    #[serde(default)]
    pub options: ResourceOptions,
}

/// Option values in requests use the text a config file accepts, such as
/// `"7 days"` or `"1 GiB"`, and the server parses them.
pub type OptionInputs = BTreeMap<String, String>;

#[derive(Debug, Clone, Serialize)]
pub struct CreateStream {
    pub name: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub options: OptionInputs,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateTopic {
    pub name: String,
    pub partitions_count: u32,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub options: OptionInputs,
}

/// A sparse `PUT` body, which the server reads as `iggy_common::OptionsUpdate`:
/// absent fields stay as they are, and `reset` returns options to the server
/// default.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct TopicUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_expiry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_topic_size: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reset: Vec<String>,
}

impl TopicUpdate {
    /// The server refuses an update that changes nothing.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CreatePartitions {
    pub partitions_count: u32,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Partition {
    pub id: u32,
    pub created_at: u64,
    pub segments_count: u32,
    pub current_offset: u64,
    pub size: String,
    pub messages_count: u64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PolledMessages {
    /// Offset of the newest message in the partition when the poll ran.
    pub current_offset: u64,
    pub messages: Vec<IggyMessage>,
}

/// `Eq` lets `Rc<IggyMessage>` props compare by pointer before comparing
/// payloads.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IggyMessage {
    pub header: IggyMessageHeader,
    /// Standard padded base64.
    pub payload: String,
    #[serde(default)]
    pub user_headers: UserHeaders,
}

/// The server sends parsed header entries, or the raw header bytes as base64
/// when they do not parse.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum UserHeaders {
    Entries(Vec<HeaderEntry>),
    Raw(String),
}

impl Default for UserHeaders {
    fn default() -> Self {
        Self::Entries(Vec::new())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HeaderEntry {
    pub key: HeaderField,
    pub value: HeaderField,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct HeaderField {
    /// A `HeaderKind` in snake case, such as `uint32`.
    pub kind: String,
    /// Standard padded base64.
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IggyMessageHeader {
    pub checksum: u64,
    pub id: u128,
    pub offset: u64,
    pub timestamp: u64,
    pub origin_timestamp: u64,
    pub payload_length: u32,
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::str::FromStr;

    use base64::Engine;
    use base64::engine::general_purpose::STANDARD;
    use iggy_common::{
        HeaderKey, HeaderValue, IggyByteSize, IggyDuration, IggyTimestamp, OptionKey,
        PartitionContext,
    };
    use serde::de::DeserializeOwned;

    use crate::changes::{MetadataChangeStatus, MetadataChanges, Position};
    use crate::permissions::{PermissionsDraft, STREAM_FLAGS, TOPIC_FLAGS};

    use super::*;

    const CREATED_AT: u64 = 1_700_000_000_000_000;
    const MESSAGE_EXPIRY_MICROS: u64 = 86_400_000_000;

    #[test]
    fn given_server_login_reply_should_decode_identity() {
        let server = iggy_common::IdentityInfo {
            user_id: 7,
            access_token: Some(iggy_common::TokenInfo {
                token: "secret".to_owned(),
                expiry: 1_800_000_000,
            }),
        };
        let mirror: IdentityInfo = decode(&server);
        assert_eq!(mirror.user_id, 7);
        assert_eq!(mirror.access_token.token, "secret");
        assert_eq!(mirror.access_token.expiry, 1_800_000_000);
    }

    #[test]
    fn given_ui_login_should_decode_as_server_login_user() {
        let mirror = LoginUser {
            username: "iggy".to_owned(),
            password: "password".to_owned(),
        };
        let server: iggy_common::login_user::LoginUser = decode(&mirror);
        assert_eq!(server.username, "iggy");
    }

    #[test]
    fn given_server_user_should_decode_user_info_details() {
        let server = iggy_common::UserInfoDetails {
            id: 1,
            created_at: IggyTimestamp::from(CREATED_AT),
            status: iggy_common::UserStatus::Active,
            username: "iggy".to_owned(),
            permissions: Some(iggy_common::Permissions {
                global: iggy_common::GlobalPermissions {
                    manage_servers: true,
                    ..Default::default()
                },
                streams: None,
            }),
            options: Default::default(),
        };
        let mirror: UserInfoDetails = decode(&server);
        assert_eq!((mirror.id, mirror.username.as_str()), (1, "iggy"));
        assert!(mirror.can_read_metadata_changes());
    }

    #[test]
    fn given_server_streams_should_decode_stream_list() {
        let server = vec![iggy_common::Stream {
            id: 1,
            created_at: IggyTimestamp::from(CREATED_AT),
            name: "orders".to_owned(),
            size: IggyByteSize::from(2_048),
            messages_count: 10,
            topics_count: 2,
            options: Default::default(),
        }];
        let mirror: Vec<Stream> = decode(&server);
        assert_eq!(
            mirror,
            [Stream {
                id: 1,
                created_at: CREATED_AT,
                name: "orders".to_owned(),
                size: "2 KiB".to_owned(),
                messages_count: 10,
                topics_count: 2,
            }]
        );
    }

    #[test]
    fn given_server_stream_details_should_decode_topics() {
        let server = iggy_common::StreamDetails {
            id: 1,
            created_at: IggyTimestamp::from(CREATED_AT),
            name: "orders".to_owned(),
            size: IggyByteSize::from(2_048),
            messages_count: 10,
            topics_count: 1,
            topics: vec![server_topic()],
            options: Default::default(),
        };
        let mirror: StreamDetails = decode(&server);
        assert_eq!(mirror.name, "orders");
        assert_eq!(mirror.topics.len(), 1);
        let topic = &mirror.topics[0];
        assert_eq!((topic.id, topic.partitions_count), (3, 1));
        assert_eq!(
            topic.options["message_expiry"].value,
            MESSAGE_EXPIRY_MICROS.to_string()
        );
    }

    #[test]
    fn given_server_topic_details_should_decode_partitions() {
        let topic = server_topic();
        let server = iggy_common::TopicDetails {
            id: topic.id,
            created_at: topic.created_at,
            name: topic.name,
            size: topic.size,
            messages_count: topic.messages_count,
            partitions_count: topic.partitions_count,
            partitions: vec![iggy_common::Partition {
                id: 0,
                created_at: IggyTimestamp::from(CREATED_AT),
                segments_count: 1,
                current_offset: 9,
                size: IggyByteSize::from(1_024),
                messages_count: 10,
                context: PartitionContext::default(),
            }],
            options: topic.options,
        };
        let mirror: TopicDetails = decode(&server);
        assert_eq!(
            mirror.partitions,
            [Partition {
                id: 0,
                created_at: CREATED_AT,
                segments_count: 1,
                current_offset: 9,
                size: "1 KiB".to_owned(),
                messages_count: 10,
            }]
        );
        assert_eq!(
            mirror.options["message_expiry"].value,
            MESSAGE_EXPIRY_MICROS.to_string()
        );
    }

    #[test]
    fn given_server_polled_messages_should_decode_payload_and_header() {
        let user_headers = BTreeMap::from([(
            HeaderKey::from_str("codec").expect("valid key"),
            HeaderValue::from_str("json").expect("valid value"),
        )]);
        let mut message = iggy_common::IggyMessage::builder()
            .id(42)
            .payload("hello".into())
            .user_headers(user_headers)
            .build()
            .expect("valid message");
        message.header.checksum = 7;
        message.header.offset = 5;
        message.header.timestamp = CREATED_AT;
        message.header.origin_timestamp = CREATED_AT - 1;
        let mut server = iggy_common::PolledMessages::empty();
        server.current_offset = 9;
        server.count = 1;
        server.messages = vec![message];

        let mirror: PolledMessages = decode(&server);
        assert_eq!(mirror.current_offset, 9);
        assert_eq!(
            mirror.messages,
            [IggyMessage {
                header: IggyMessageHeader {
                    checksum: 7,
                    id: 42,
                    offset: 5,
                    timestamp: CREATED_AT,
                    origin_timestamp: CREATED_AT - 1,
                    payload_length: 5,
                },
                payload: STANDARD.encode("hello"),
                user_headers: UserHeaders::Entries(vec![HeaderEntry {
                    key: HeaderField {
                        kind: "string".to_owned(),
                        value: STANDARD.encode("codec"),
                    },
                    value: HeaderField {
                        kind: "string".to_owned(),
                        value: STANDARD.encode("json"),
                    },
                }]),
            }]
        );
    }

    #[test]
    fn given_server_stats_should_decode_overview_fields() {
        let server = iggy_common::Stats {
            process_id: 42,
            cpu_usage: 1.5,
            memory_usage: IggyByteSize::from(2_048),
            run_time: IggyDuration::from(90_000_000),
            start_time: IggyTimestamp::from(CREATED_AT),
            messages_count: 10,
            hostname: "node-1".to_owned(),
            iggy_server_version: "0.11.1".to_owned(),
            open_files_count: 3,
            open_files_limit: 1_024,
            ..Default::default()
        };
        let mirror: Stats = decode(&server);
        assert_eq!(mirror.process_id, 42);
        assert_eq!(mirror.cpu_usage, 1.5);
        assert_eq!(mirror.memory_usage, "2 KiB");
        assert_eq!(mirror.run_time, 90_000_000);
        assert_eq!(mirror.start_time, CREATED_AT);
        assert_eq!(mirror.messages_count, 10);
        assert_eq!(mirror.hostname, "node-1");
        assert_eq!(mirror.iggy_server_version, "0.11.1");
        assert_eq!(
            (mirror.open_files_count, mirror.open_files_limit),
            (3, 1_024)
        );
    }

    #[test]
    fn given_ui_create_stream_should_decode_as_server_create_stream() {
        let mirror = CreateStream {
            name: "orders".to_owned(),
            options: OptionInputs::from([("encryption_key".to_owned(), "server".to_owned())]),
        };
        let server: iggy_common::create_stream::CreateStream = decode(&mirror);
        assert_eq!(server.name, "orders");
        assert_eq!(
            server.options.0.keys().collect::<Vec<_>>(),
            ["encryption_key"]
        );
    }

    #[test]
    fn given_ui_create_topic_should_decode_as_server_create_topic() {
        let mirror = CreateTopic {
            name: "events".to_owned(),
            partitions_count: 3,
            options: OptionInputs::from([
                ("compression_algorithm".to_owned(), "gzip".to_owned()),
                ("max_topic_size".to_owned(), "1 GiB".to_owned()),
                ("message_expiry".to_owned(), "7 days".to_owned()),
            ]),
        };
        let server: iggy_common::create_topic::CreateTopic = decode(&mirror);
        assert_eq!(
            (server.name.as_str(), server.partitions_count),
            ("events", 3)
        );
        server
            .options
            .to_wire()
            .expect("the server accepts the topic options the form sends");
    }

    #[test]
    fn given_ui_topic_update_should_decode_as_server_options_update() {
        let mirror = TopicUpdate {
            name: Some("renamed".to_owned()),
            message_expiry: Some("1 day".to_owned()),
            max_topic_size: None,
            reset: vec!["max_topic_size".to_owned()],
        };
        let server: iggy_common::OptionsUpdate = decode(&mirror);
        assert_eq!(
            server.set.keys().collect::<Vec<_>>(),
            ["message_expiry", "name"]
        );
        assert_eq!(server.reset.iter().collect::<Vec<_>>(), ["max_topic_size"]);
        server
            .to_patch()
            .expect("the server accepts the topic update the form sends");
    }

    #[test]
    fn given_ui_create_partitions_should_decode_as_server_create_partitions() {
        let server: iggy_common::create_partitions::CreatePartitions = decode(&CreatePartitions {
            partitions_count: 2,
        });
        assert_eq!(server.partitions_count, 2);
    }

    #[test]
    fn given_server_users_should_decode_user_list() {
        let server = vec![iggy_common::UserInfo {
            id: 2,
            created_at: IggyTimestamp::from(CREATED_AT),
            status: iggy_common::UserStatus::Inactive,
            username: "reader".to_owned(),
            options: Default::default(),
        }];
        let mirror: Vec<UserInfo> = decode(&server);
        assert_eq!(
            mirror,
            [UserInfo {
                id: 2,
                created_at: CREATED_AT,
                status: UserStatus::Inactive,
                username: "reader".to_owned(),
            }]
        );
    }

    #[test]
    fn given_ui_create_user_should_decode_as_server_create_user() {
        let mut draft = PermissionsDraft::default();
        draft.set_global("manage_servers", true);
        draft.set_stream(1, STREAM_FLAGS[0].key, true);
        draft.set_topic(1, 2, TOPIC_FLAGS[0].key, true);
        let mirror = CreateUser {
            username: "reader".to_owned(),
            password: "secret123".to_owned(),
            status: UserStatus::Active,
            permissions: draft.to_json(),
        };
        let server: iggy_common::create_user::CreateUser = decode(&mirror);
        assert_eq!(server.username, "reader");
        assert_eq!(server.status, iggy_common::UserStatus::Active);
        assert_eq!(
            serde_json::to_value(&server.permissions).expect("serializable"),
            draft.to_json(),
            "the server reads every permission flag the form sends"
        );
    }

    #[test]
    fn given_ui_user_update_should_decode_as_server_options_update() {
        let mirror = UserUpdate {
            username: Some("renamed".to_owned()),
            status: Some(UserStatus::Inactive),
        };
        let server: iggy_common::OptionsUpdate = decode(&mirror);
        assert_eq!(
            server.set.keys().collect::<Vec<_>>(),
            ["status", "username"]
        );
        server
            .to_patch()
            .expect("the server accepts the user update the form sends");
    }

    #[test]
    fn given_status_name_should_round_trip() {
        for status in UserStatus::ALL {
            assert_eq!(status.as_str().parse(), Ok(status));
        }
        assert_eq!(
            "banned".parse::<UserStatus>(),
            Err(UnknownUserStatus("banned".to_owned()))
        );
    }

    #[test]
    fn given_server_change_page_should_decode_metadata_changes() {
        let log_id = 0x0d4b_8f2a_6e1c_93d7;
        let cursor = iggy_common::MetadataChangeCursor {
            log_id,
            op: 42,
            checksum: 0xf2a4,
            ordinal: 1,
        };
        let server = iggy_common::MetadataChanges {
            status: iggy_common::MetadataChangeStatus::Ok,
            phase: iggy_common::MetadataChangePhase::Changes,
            log_id: Some(log_id),
            oldest_op: 1,
            retained_since: 0,
            head_op: 42,
            next: Some(cursor),
            has_more: true,
            records: vec![iggy_common::MetadataChangeRecord {
                op: 42,
                ordinal: 1,
                checksum: 0xf2a4,
                timestamp: CREATED_AT,
                actor_user_id: Some(0),
                change: iggy_common::MetadataChange::UserDeleted(
                    iggy_common::MetadataUserIdentity {
                        user_id: 2,
                        user_created_at: CREATED_AT,
                        username: "reader".to_owned(),
                    },
                ),
            }],
            items: vec![],
            current_page_token: None,
        };
        let mirror: MetadataChanges = decode(&server);
        assert_eq!(mirror.status, MetadataChangeStatus::Ok);
        assert_eq!(mirror.log_id, Some(format!("{log_id:032x}")));
        assert_eq!((mirror.head_op, mirror.has_more), (42, true));
        let next: Position = mirror
            .next
            .expect("a cursor")
            .parse()
            .expect("the UI reads the cursor");
        assert_eq!((next.op, next.ordinal), (42, 1));
        let [record] = mirror.records.as_slice() else {
            panic!("one record, got {}", mirror.records.len());
        };
        assert_eq!((record.op, record.ordinal), (42, 1));
        assert_eq!(
            (record.timestamp, record.actor_user_id),
            (CREATED_AT, Some(0))
        );
        assert_eq!(
            (record.group.as_str(), record.kind.as_str()),
            ("users", "user_deleted")
        );
        assert_eq!(record.summary(), "user \"reader\" (2)");
    }

    fn server_topic() -> iggy_common::Topic {
        let options = iggy_common::ResourceOptions::from([(
            HeaderKey::from(OptionKey::MessageExpiry),
            iggy_common::OptionValue::explicit(HeaderValue::from(MESSAGE_EXPIRY_MICROS)),
        )]);
        iggy_common::Topic {
            id: 3,
            created_at: IggyTimestamp::from(CREATED_AT),
            name: "events".to_owned(),
            size: IggyByteSize::from(1_024),
            messages_count: 10,
            partitions_count: 1,
            options,
        }
    }

    fn decode<T: DeserializeOwned>(value: &impl Serialize) -> T {
        let json = serde_json::to_string(value).expect("serializable");
        serde_json::from_str(&json).expect("the mirror decodes the server's JSON")
    }
}
