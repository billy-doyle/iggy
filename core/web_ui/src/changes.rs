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

//! The server's metadata change log, `GET /metadata/changes`: query
//! building, positions, and the "newest first" scan.

use std::collections::BTreeSet;
use std::fmt;
use std::future::Future;
use std::str::FromStr;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const DEFAULT_LIMIT: u32 = 100;
pub const MAX_LIMIT: u32 = 1000;
/// The most records the page keeps in memory.
pub const MAX_SHOWN: usize = 5000;

/// Query parameters for `GET /metadata/changes`.
pub type Query = Vec<(&'static str, String)>;

const CURSOR_PREFIX: &str = "mc1_";
const CURSOR_BYTES: usize = 44;

/// The change classes the server can filter on, in the order it lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetadataChangeGroup {
    Streams,
    Topics,
    Partitions,
    ConsumerGroups,
    ConsumerGroupMembers,
    Users,
    PersonalAccessTokens,
    Sessions,
}

impl MetadataChangeGroup {
    pub const ALL: [Self; 8] = [
        Self::Streams,
        Self::Topics,
        Self::Partitions,
        Self::ConsumerGroups,
        Self::ConsumerGroupMembers,
        Self::Users,
        Self::PersonalAccessTokens,
        Self::Sessions,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Streams => "streams",
            Self::Topics => "topics",
            Self::Partitions => "partitions",
            Self::ConsumerGroups => "consumer_groups",
            Self::ConsumerGroupMembers => "consumer_group_members",
            Self::Users => "users",
            Self::PersonalAccessTokens => "personal_access_tokens",
            Self::Sessions => "sessions",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataChangeStatus {
    Ok,
    /// This node has not applied the requested position yet.
    Behind,
    /// The node no longer holds every change after the position.
    Expired,
    /// The position belongs to another history.
    Reset,
}

impl MetadataChangeStatus {
    /// What to tell the reader about a page that carries no records.
    pub fn message(self, following: bool) -> Option<&'static str> {
        match self {
            Self::Ok => None,
            Self::Behind if following => {
                Some("This node has not applied that position yet. Retrying.")
            }
            Self::Behind => {
                Some("This node has not applied that position yet. Load again shortly.")
            }
            Self::Expired => Some(
                "The node no longer holds every change after this position. Load again from the earliest retained change.",
            ),
            Self::Reset => Some(
                "The position belongs to another history, for example a rebuilt data directory. Load again.",
            ),
        }
    }
}

/// One change. The fields every record carries are typed, and the rest,
/// which depend on the kind, stay as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataChangeRecord {
    pub op: u64,
    pub ordinal: u32,
    pub checksum: String,
    /// Unix microseconds.
    pub timestamp: u64,
    /// Absent when the server itself made the change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_user_id: Option<u32>,
    pub group: String,
    pub kind: String,
    #[serde(flatten)]
    pub fields: Map<String, Value>,
}

impl MetadataChangeRecord {
    pub fn position(&self) -> Position {
        Position {
            op: self.op,
            ordinal: self.ordinal,
        }
    }

    /// The kind as words, `stream_created` as "stream created".
    pub fn kind_label(&self) -> String {
        self.kind.replace('_', " ")
    }

    /// One line naming what the record changed.
    pub fn summary(&self) -> String {
        let text = |field: &str| match self.fields.get(field) {
            Some(Value::String(value)) => value.clone(),
            Some(value) => value.to_string(),
            None => "?".to_owned(),
        };
        let topic = format!("topic {}/{}", text("stream_id"), text("topic_id"));
        let group = format!("group {} on {topic}", text("group_id"));
        match self.group.as_str() {
            "streams" => format!("stream \"{}\" ({})", text("name"), text("stream_id")),
            "topics" => format!("{topic} \"{}\"", text("name")),
            "partitions" => format!("{topic} now has {} partitions", text("partitions_count")),
            "consumer_groups" => format!("{group} \"{}\"", text("name")),
            "consumer_group_members" if self.kind == "consumer_group_partition_handed_over" => {
                let owner = match self.fields.get("member_id") {
                    Some(_) => format!("member {}", text("member_id")),
                    None => "no member".to_owned(),
                };
                format!("partition {} of {group} to {owner}", text("partition_id"))
            }
            "consumer_group_members" => format!("member {} of {group}", text("member_id")),
            "users" => format!("user \"{}\" ({})", text("username"), text("user_id")),
            "personal_access_tokens" => {
                format!("token \"{}\" of user {}", text("name"), text("user_id"))
            }
            "sessions" => format!("session {} of user {}", text("session"), text("user_id")),
            other => format!("{other} {}", self.kind),
        }
    }
}

/// One page of the log. Like the mirrors in `models.rs`, its test decodes
/// the server's serialized `iggy_common::MetadataChanges`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct MetadataChanges {
    pub status: MetadataChangeStatus,
    /// The history the positions belong to, absent before the first op.
    pub log_id: Option<String>,
    pub head_op: u64,
    /// The cursor to continue after.
    pub next: Option<String>,
    pub has_more: bool,
    #[serde(default)]
    pub records: Vec<MetadataChangeRecord>,
}

/// Where a read starts when it has no cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Start {
    Earliest,
    /// The current end of the log, which returns a cursor and no records.
    Latest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangesFilter {
    /// Empty means every group.
    pub groups: BTreeSet<MetadataChangeGroup>,
    /// Unix microseconds.
    pub since: Option<u64>,
    pub until: Option<u64>,
    pub limit: u32,
}

impl ChangesFilter {
    /// Why the server would refuse this filter.
    pub fn validate(&self) -> Result<(), FilterError> {
        if !(1..=MAX_LIMIT).contains(&self.limit) {
            return Err(FilterError::LimitOutOfRange);
        }
        if let (Some(since), Some(until)) = (self.since, self.until)
            && since > until
        {
            return Err(FilterError::InvertedRange);
        }
        Ok(())
    }

    /// Query parameters of `GET /metadata/changes`, continuing after
    /// `cursor` when given.
    pub fn query(&self, start: Start, cursor: Option<&str>) -> Query {
        let mut query = match cursor {
            Some(cursor) => vec![("after", cursor.to_owned())],
            None => vec![("from", start.as_str().to_owned())],
        };
        if !self.groups.is_empty() {
            let groups: Vec<&str> = self.groups.iter().map(|group| group.as_str()).collect();
            query.push(("groups", groups.join(",")));
        }
        // The server refuses time bounds on a read that starts at the end.
        if cursor.is_some() || start == Start::Earliest {
            if let Some(since) = self.since {
                query.push(("since", since.to_string()));
            }
            if let Some(until) = self.until {
                query.push(("until", until.to_string()));
            }
        }
        query.push(("limit", self.limit.to_string()));
        query
    }
}

impl Start {
    fn as_str(self) -> &'static str {
        match self {
            Self::Earliest => "earliest",
            Self::Latest => "latest",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FilterError {
    #[error("Page size must be a whole number from 1 to {MAX_LIMIT}")]
    LimitOutOfRange,
    #[error("The start of the time range is after its end")]
    InvertedRange,
}

/// Unix microseconds of a `datetime-local` value read as UTC, `None` when
/// empty or unreadable.
pub fn micros_from_utc_input(value: &str) -> Option<u64> {
    let value = value.trim();
    ["%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"]
        .iter()
        .find_map(|pattern| NaiveDateTime::parse_from_str(value, pattern).ok())
        .and_then(|time| u64::try_from(time.and_utc().timestamp_micros()).ok())
}

/// A record's place in the log. Positions order by op, then by ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub op: u64,
    pub ordinal: u32,
}

impl FromStr for Position {
    type Err = InvalidCursor;

    /// Reads the position out of a cursor: `mc1_` and unpadded base64url of
    /// `[log_id u128][op u64][checksum u128][ordinal u32]`, little-endian.
    fn from_str(cursor: &str) -> Result<Self, Self::Err> {
        let encoded = cursor.strip_prefix(CURSOR_PREFIX).ok_or(InvalidCursor)?;
        let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| InvalidCursor)?;
        if bytes.len() != CURSOR_BYTES {
            return Err(InvalidCursor);
        }
        let op = u64::from_le_bytes(bytes[16..24].try_into().map_err(|_| InvalidCursor)?);
        let ordinal = u32::from_le_bytes(bytes[40..44].try_into().map_err(|_| InvalidCursor)?);
        Ok(Self { op, ordinal })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("The server returned an invalid change position.")]
pub struct InvalidCursor;

/// The newest records up to the end of the log as it was when the scan
/// started, at most `capacity` of them.
#[derive(Debug, Clone, PartialEq)]
pub struct RecentChanges {
    pub page: MetadataChanges,
    /// Older matching records were dropped to stay within capacity.
    pub trimmed: bool,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ScanError<E: fmt::Display> {
    #[error("{0}")]
    Read(E),
    #[error(transparent)]
    InvalidCursor(#[from] InvalidCursor),
    #[error("The server did not advance the change position. Load again.")]
    Stalled,
}

/// Reads "newest first". The log only pages forward, so this captures the
/// current end, then reads from the earliest retained change up to it,
/// keeping the newest `capacity` records that match the filter.
pub async fn scan_recent<E, Read, Page>(
    filter: &ChangesFilter,
    capacity: usize,
    mut read_page: Read,
    mut on_page: impl FnMut(usize),
) -> Result<RecentChanges, ScanError<E>>
where
    E: fmt::Display,
    Read: FnMut(Query) -> Page,
    Page: Future<Output = Result<MetadataChanges, E>>,
{
    let boundary = read_page(filter.query(Start::Latest, None))
        .await
        .map_err(ScanError::Read)?;
    let Some(boundary_cursor) = boundary
        .next
        .clone()
        .filter(|_| boundary.status == MetadataChangeStatus::Ok)
    else {
        return Ok(RecentChanges {
            page: boundary,
            trimmed: false,
        });
    };
    let end: Position = boundary_cursor.parse()?;
    let page_filter = ChangesFilter {
        limit: MAX_LIMIT,
        ..filter.clone()
    };
    let mut after: Option<String> = None;
    let mut records: Vec<MetadataChangeRecord> = Vec::new();
    let mut trimmed = false;
    let mut pages = 0;
    loop {
        let mut page = read_page(page_filter.query(Start::Earliest, after.as_deref()))
            .await
            .map_err(ScanError::Read)?;
        pages += 1;
        on_page(pages);
        if page.status != MetadataChangeStatus::Ok {
            page.records.clear();
            return Ok(RecentChanges {
                page,
                trimmed: false,
            });
        }
        if page.log_id != boundary.log_id {
            page.status = MetadataChangeStatus::Reset;
            page.records.clear();
            page.next = None;
            page.has_more = false;
            return Ok(RecentChanges {
                page,
                trimmed: false,
            });
        }
        let matching = page
            .records
            .drain(..)
            .filter(|record| record.position() <= end);
        records.extend(matching);
        if records.len() > capacity {
            trimmed = true;
            records.drain(..records.len() - capacity);
        }
        let passed_end = match &page.next {
            Some(next) => next.parse::<Position>()? >= end,
            None => false,
        };
        if !page.has_more || passed_end {
            // A node that has not applied the captured end stops short of
            // it, so it keeps its own position.
            if passed_end || page.head_op >= end.op {
                page.head_op = boundary.head_op;
                page.next = Some(boundary_cursor);
            }
            page.records = records;
            page.has_more = false;
            return Ok(RecentChanges { page, trimmed });
        }
        match page.next {
            Some(next) if Some(&next) != after.as_ref() => after = Some(next),
            _ => return Err(ScanError::Stalled),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use futures::executor::block_on;
    use serde_json::json;

    use super::*;

    const LOG: &str = "00000000000000003184b12a90beb20e";

    fn cursor(op: u64, ordinal: u32) -> String {
        let mut bytes = Vec::with_capacity(CURSOR_BYTES);
        bytes.extend_from_slice(&0x3184_b12a_90be_b20e_u128.to_le_bytes());
        bytes.extend_from_slice(&op.to_le_bytes());
        bytes.extend_from_slice(&0_u128.to_le_bytes());
        bytes.extend_from_slice(&ordinal.to_le_bytes());
        format!("{CURSOR_PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes))
    }

    fn record(op: u64, ordinal: u32) -> MetadataChangeRecord {
        serde_json::from_value(json!({
            "op": op, "ordinal": ordinal, "checksum": "00", "timestamp": op * 1_000,
            "group": "streams", "kind": "stream_created", "stream_id": op, "name": format!("s{op}"),
        }))
        .unwrap()
    }

    fn page(
        records: Vec<MetadataChangeRecord>,
        next: Option<String>,
        has_more: bool,
        head_op: u64,
    ) -> MetadataChanges {
        MetadataChanges {
            status: MetadataChangeStatus::Ok,
            log_id: Some(LOG.to_owned()),
            head_op,
            next,
            has_more,
            records,
        }
    }

    fn filter() -> ChangesFilter {
        ChangesFilter {
            groups: BTreeSet::new(),
            since: None,
            until: None,
            limit: DEFAULT_LIMIT,
        }
    }

    /// Runs a scan that answers each request with the next scripted page,
    /// and returns its result with the queries it sent.
    fn scan(
        filter: &ChangesFilter,
        capacity: usize,
        pages: Vec<MetadataChanges>,
    ) -> (Result<RecentChanges, ScanError<String>>, Vec<Query>) {
        let answers = RefCell::new(VecDeque::from(pages));
        let queries = RefCell::new(Vec::new());
        let result = block_on(scan_recent(
            filter,
            capacity,
            |query| {
                queries.borrow_mut().push(query);
                let answer = answers
                    .borrow_mut()
                    .pop_front()
                    .ok_or_else(|| "no more pages".to_owned());
                async move { answer }
            },
            |_| {},
        ));
        (result, queries.into_inner())
    }

    #[test]
    fn given_earliest_filter_when_building_query_should_send_groups_in_code_order_and_bounds() {
        let filter = ChangesFilter {
            groups: BTreeSet::from([MetadataChangeGroup::Users, MetadataChangeGroup::Streams]),
            since: Some(10),
            until: Some(20),
            limit: 50,
        };
        assert_eq!(
            filter.query(Start::Earliest, None),
            vec![
                ("from", "earliest".to_owned()),
                ("groups", "streams,users".to_owned()),
                ("since", "10".to_owned()),
                ("until", "20".to_owned()),
                ("limit", "50".to_owned()),
            ]
        );
    }

    #[test]
    fn given_cursor_when_building_query_should_continue_after_it_and_keep_bounds() {
        let filter = ChangesFilter {
            since: Some(10),
            ..filter()
        };
        let query = filter.query(Start::Latest, Some("mc1_x"));
        assert_eq!(query[0], ("after", "mc1_x".to_owned()));
        assert!(query.contains(&("since", "10".to_owned())));
    }

    #[test]
    fn given_latest_filter_when_building_query_should_drop_time_bounds_the_server_refuses() {
        let filter = ChangesFilter {
            since: Some(10),
            until: Some(20),
            ..filter()
        };
        let query = filter.query(Start::Latest, None);
        assert_eq!(
            query,
            vec![("from", "latest".to_owned()), ("limit", "100".to_owned())]
        );
    }

    #[test]
    fn given_out_of_range_limit_or_inverted_range_when_validating_should_name_the_problem() {
        assert_eq!(
            ChangesFilter {
                limit: 0,
                ..filter()
            }
            .validate(),
            Err(FilterError::LimitOutOfRange)
        );
        assert_eq!(
            ChangesFilter {
                limit: 1001,
                ..filter()
            }
            .validate(),
            Err(FilterError::LimitOutOfRange)
        );
        assert_eq!(
            ChangesFilter {
                since: Some(2),
                until: Some(1),
                ..filter()
            }
            .validate(),
            Err(FilterError::InvertedRange)
        );
        assert_eq!(
            ChangesFilter {
                limit: 1000,
                ..filter()
            }
            .validate(),
            Ok(())
        );
    }

    #[test]
    fn given_datetime_local_when_converting_should_read_it_as_utc_microseconds() {
        assert_eq!(
            micros_from_utc_input("2023-11-14T22:13"),
            Some(1_699_999_980_000_000)
        );
        assert_eq!(
            micros_from_utc_input("2023-11-14T22:13:20"),
            Some(1_700_000_000_000_000)
        );
        assert_eq!(micros_from_utc_input(""), None);
        assert_eq!(micros_from_utc_input("yesterday"), None);
    }

    #[test]
    fn given_non_ok_status_when_describing_should_explain_what_to_do() {
        assert_eq!(MetadataChangeStatus::Ok.message(false), None);
        assert!(
            MetadataChangeStatus::Expired
                .message(false)
                .unwrap()
                .contains("earliest retained")
        );
        assert!(
            MetadataChangeStatus::Reset
                .message(false)
                .unwrap()
                .contains("another history")
        );
    }

    #[test]
    fn given_behind_status_when_describing_should_promise_a_retry_only_while_following() {
        assert!(
            MetadataChangeStatus::Behind
                .message(true)
                .unwrap()
                .ends_with("Retrying.")
        );
        assert!(
            MetadataChangeStatus::Behind
                .message(false)
                .unwrap()
                .ends_with("Load again shortly.")
        );
    }

    #[test]
    fn given_records_when_describing_should_name_the_changed_resource() {
        let describe = |value: Value| {
            serde_json::from_value::<MetadataChangeRecord>(value)
                .unwrap()
                .summary()
        };
        let base = |group: &str, kind: &str| json!({"op": 1, "ordinal": 0, "checksum": "0", "timestamp": 1, "group": group, "kind": kind});
        let with = |mut base: Value, extra: Value| {
            base.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            base
        };
        assert_eq!(
            describe(with(
                base("streams", "stream_created"),
                json!({"stream_id": 3, "name": "orders"})
            )),
            "stream \"orders\" (3)"
        );
        assert_eq!(
            describe(with(
                base("partitions", "partitions_created"),
                json!({"stream_id": 1, "topic_id": 2, "partitions_count": 4})
            )),
            "topic 1/2 now has 4 partitions"
        );
        assert_eq!(
            describe(with(
                base(
                    "consumer_group_members",
                    "consumer_group_partition_handed_over"
                ),
                json!({"stream_id": 1, "topic_id": 2, "group_id": 5, "partition_id": 0})
            )),
            "partition 0 of group 5 on topic 1/2 to no member"
        );
        assert_eq!(
            describe(with(
                base("users", "user_created"),
                json!({"user_id": 0, "username": "iggy"})
            )),
            "user \"iggy\" (0)"
        );
        assert_eq!(
            describe(base("streams", "stream_deleted")),
            "stream \"?\" (?)"
        );
        assert_eq!(
            describe(base("widgets", "widget_made")),
            "widgets widget_made"
        );
    }

    #[test]
    fn given_server_cursor_when_parsing_should_read_op_and_ordinal() {
        assert_eq!(
            "mc1_DrK-kCqxhDEAAAAAAAAAAAMAAAAAAAAADVfgzmEPgB8AAAAAAAAAAAAAAAA".parse(),
            Ok(Position { op: 3, ordinal: 0 })
        );
        assert_eq!(cursor(7, 2).parse(), Ok(Position { op: 7, ordinal: 2 }));
        assert_eq!("mc2_AAAA".parse::<Position>(), Err(InvalidCursor));
        assert_eq!("mc1_AAAA".parse::<Position>(), Err(InvalidCursor));
    }

    #[test]
    fn given_record_when_serializing_should_round_trip_kind_specific_fields() {
        let original = json!({"op": 3, "ordinal": 0, "checksum": "ab", "timestamp": 5, "actor_user_id": 0,
            "group": "streams", "kind": "stream_created", "stream_id": 0, "options": {"x": {"value": "1"}}});
        let record: MetadataChangeRecord = serde_json::from_value(original.clone()).unwrap();
        assert_eq!(record.actor_user_id, Some(0));
        assert_eq!(serde_json::to_value(&record).unwrap(), original);
    }

    #[test]
    fn given_recent_scan_when_pages_cross_the_captured_head_should_keep_only_the_newest_bounded_records()
     {
        let pages = vec![
            page(vec![], Some(cursor(4, 0)), false, 4),
            page(
                vec![record(1, 0), record(2, 0)],
                Some(cursor(2, 0)),
                true,
                6,
            ),
            page(
                vec![record(3, 0), record(4, 0), record(5, 0)],
                Some(cursor(5, 0)),
                true,
                6,
            ),
        ];
        let (result, queries) = scan(&filter(), 2, pages);
        let recent = result.unwrap();
        let ops: Vec<u64> = recent.page.records.iter().map(|record| record.op).collect();
        assert_eq!(ops, [3, 4]);
        assert!(recent.trimmed);
        assert_eq!(recent.page.next, Some(cursor(4, 0)));
        assert_eq!(recent.page.head_op, 4);
        assert!(!recent.page.has_more);
        assert_eq!(queries.len(), 3);
        assert_eq!(queries[1][0], ("from", "earliest".to_owned()));
        assert!(queries[1].contains(&("limit", MAX_LIMIT.to_string())));
        assert_eq!(queries[2][0], ("after", cursor(2, 0)));
    }

    #[test]
    fn given_empty_filtered_pages_when_scanning_recent_should_stop_at_the_captured_cursor_without_waiting_for_new_writes()
     {
        let pages = vec![
            page(vec![], Some(cursor(9, 0)), false, 9),
            page(vec![], Some(cursor(9, 0)), true, 9),
        ];
        let (result, queries) = scan(&filter(), 10, pages);
        let recent = result.unwrap();
        assert!(recent.page.records.is_empty());
        assert_eq!(recent.page.next, Some(cursor(9, 0)));
        assert_eq!(queries.len(), 2);
    }

    #[test]
    fn given_node_behind_the_captured_head_when_scanning_recent_should_continue_from_its_own_position()
     {
        let pages = vec![
            page(vec![], Some(cursor(9, 0)), false, 9),
            page(vec![record(1, 0)], Some(cursor(1, 0)), false, 1),
        ];
        let (result, _) = scan(&filter(), 10, pages);
        let recent = result.unwrap();
        assert_eq!(recent.page.next, Some(cursor(1, 0)));
        assert_eq!(recent.page.head_op, 1);
        assert_eq!(recent.page.records.len(), 1);
    }

    #[test]
    fn given_expiry_or_changed_history_during_recent_scan_should_discard_partial_results() {
        let mut expired = page(vec![record(1, 0)], None, false, 9);
        expired.status = MetadataChangeStatus::Expired;
        let (result, _) = scan(
            &filter(),
            10,
            vec![page(vec![], Some(cursor(9, 0)), false, 9), expired],
        );
        let recent = result.unwrap();
        assert_eq!(recent.page.status, MetadataChangeStatus::Expired);
        assert!(recent.page.records.is_empty());

        let mut other_history = page(vec![record(1, 0)], Some(cursor(1, 0)), true, 9);
        other_history.log_id = Some("ff".to_owned());
        let (result, _) = scan(
            &filter(),
            10,
            vec![page(vec![], Some(cursor(9, 0)), false, 9), other_history],
        );
        let recent = result.unwrap();
        assert_eq!(recent.page.status, MetadataChangeStatus::Reset);
        assert!(recent.page.records.is_empty());
        assert_eq!(recent.page.next, None);
    }

    #[test]
    fn given_recent_time_filters_when_capturing_head_should_omit_bounds_only_for_latest() {
        let filter = ChangesFilter {
            since: Some(5),
            ..filter()
        };
        let pages = vec![
            page(vec![], Some(cursor(2, 0)), false, 2),
            page(vec![], Some(cursor(2, 0)), false, 2),
        ];
        let (_, queries) = scan(&filter, 10, pages);
        assert!(!queries[0].iter().any(|(key, _)| *key == "since"));
        assert!(queries[1].contains(&("since", "5".to_owned())));
    }

    #[test]
    fn given_server_that_does_not_advance_when_scanning_recent_should_stop_with_an_error() {
        let pages = vec![
            page(vec![], Some(cursor(9, 0)), false, 9),
            page(vec![], Some(cursor(1, 0)), true, 9),
            page(vec![], Some(cursor(1, 0)), true, 9),
        ];
        let (result, _) = scan(&filter(), 10, pages);
        assert_eq!(result, Err(ScanError::Stalled));
    }
}
