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
//! the fields it renders. Unknown fields are ignored on purpose.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IdentityInfo {
    pub user_id: u32,
    pub access_token: Option<TokenInfo>,
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

impl std::str::FromStr for UserStatus {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_str() == text)
            .ok_or_else(|| format!("unknown user status {text}"))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateUser {
    pub username: String,
    pub password: String,
    pub status: UserStatus,
    /// Built by [`crate::permissions::PermissionsDraft::to_json`].
    pub permissions: serde_json::Value,
}

/// A sparse `PUT /users/{id}` body.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct UserUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<UserStatus>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct UserDetails {
    pub id: u32,
    pub username: String,
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
    pub kind: String,
    pub explicit: bool,
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

/// A sparse `PUT` body: absent fields stay as they are, and `reset` returns
/// options to the server default.
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
pub struct PartitionsCount {
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
    pub partition_id: u32,
    pub current_offset: u64,
    pub count: u32,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Message {
    pub header: MessageHeader,
    /// Standard padded base64.
    pub payload: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MessageHeader {
    pub checksum: u64,
    pub id: u128,
    pub offset: u64,
    pub timestamp: u64,
    pub origin_timestamp: u64,
    pub user_headers_length: u32,
    pub payload_length: u32,
}
