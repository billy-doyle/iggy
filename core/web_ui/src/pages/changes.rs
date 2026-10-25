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

use std::collections::BTreeSet;
use std::rc::Rc;

use dioxus::core::Task;
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use serde_json::Value;

use crate::api::ApiError;
use crate::auth::Session;
use crate::changes::{
    self, ChangesFilter, DEFAULT_LIMIT, FilterError, MAX_SHOWN, MetadataChangeGroup,
    MetadataChangeRecord, MetadataChangeStatus, ScanError, Start,
};
use crate::components::{Api, CurrentUser, Loading, Modal, PageHeader, use_api, use_session};
use crate::format;
use crate::models::UserInfoDetails;

const FOLLOW_INTERVAL_MS: u32 = 2_000;
const COPY_FEEDBACK_MS: u32 = 3_000;
const NOT_ALLOWED: &str =
    "Reading metadata changes needs the root user or the global manage servers permission.";

#[component]
pub fn Changes() -> Element {
    let CurrentUser(user) = use_context();
    let allowed = match &*user.read() {
        None => {
            return rsx! {
                PageHeader { title: "Metadata changes" }
                Loading {}
            };
        }
        Some(user) => user
            .as_ref()
            .is_some_and(UserInfoDetails::can_read_metadata_changes),
    };

    rsx! {
        PageHeader { title: "Metadata changes" }
        if allowed {
            ChangeLog {}
        } else {
            p { class: "muted", "{NOT_ALLOWED}" }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Order {
    OldestFirst,
    /// Scans to the current end of the log and shows the newest records.
    NewestFirst,
}

impl Order {
    const ALL: [Self; 2] = [Self::NewestFirst, Self::OldestFirst];

    fn as_str(self) -> &'static str {
        match self {
            Self::OldestFirst => "oldest",
            Self::NewestFirst => "newest",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::OldestFirst => "Oldest first",
            Self::NewestFirst => "Newest first",
        }
    }
}

/// What the page has loaded and is doing.
#[derive(Debug, Default)]
struct LogView {
    /// Shared, so rendering a row clones a pointer and an unchanged row
    /// compares equal by pointer.
    records: Vec<Rc<MetadataChangeRecord>>,
    cursor: Option<String>,
    has_more: bool,
    loaded: bool,
    /// How many records "newest first" shows. That scan loads everything
    /// at once, so Show more reveals records instead of reading a page.
    visible: Option<usize>,
    scanned_pages: usize,
    notice: Option<String>,
    problem: Option<String>,
    forbidden: bool,
    loading: bool,
    following: bool,
    /// The filter the loaded records came from, used to continue reading.
    filter: Option<ChangesFilter>,
}

impl LogView {
    /// "Newest first" shows its first `visible` records, newest at the top.
    fn shown(&self) -> Vec<&Rc<MetadataChangeRecord>> {
        match self.visible {
            Some(visible) => self.records.iter().rev().take(visible).collect(),
            None => self.records.iter().collect(),
        }
    }
}

#[component]
fn ChangeLog() -> Element {
    let api = use_api();
    let session = use_session();
    let mut order = use_signal(|| Order::OldestFirst);
    let mut since = use_signal(String::new);
    let mut until = use_signal(String::new);
    let mut limit = use_signal(|| DEFAULT_LIMIT.to_string());
    let mut groups = use_signal(BTreeSet::<MetadataChangeGroup>::new);
    let mut view = use_signal(LogView::default);
    let mut scan_task = use_signal(|| None::<Task>);

    let mut load = move || {
        let filter = match read_filter(&since.peek(), &until.peek(), &limit.peek(), &groups.peek())
        {
            Ok(filter) => filter,
            Err(invalid) => {
                view.write().problem = Some(invalid.to_string());
                return;
            }
        };
        if let Some(task) = scan_task.take() {
            task.cancel();
        }
        let newest_first = *order.peek() == Order::NewestFirst;
        *view.write() = LogView {
            visible: newest_first.then_some(filter.limit as usize),
            filter: Some(filter.clone()),
            loading: true,
            ..LogView::default()
        };
        if newest_first {
            scan_task.set(Some(spawn(scan_newest(
                api, session, view, scan_task, filter,
            ))));
        } else {
            spawn(read_page(api, session, view, filter, None));
        }
    };

    use_hook(load);

    // Following polls from a loop that checks a flag, so stopping never has
    // to cancel the task that is doing the stopping.
    use_future(move || async move {
        loop {
            TimeoutFuture::new(FOLLOW_INTERVAL_MS).await;
            let next = {
                let current = view.peek();
                (current.following && !current.loading)
                    .then(|| current.filter.clone().zip(current.cursor.clone()))
                    .flatten()
            };
            if let Some((filter, cursor)) = next {
                read_page(api, session, view, filter, Some(cursor)).await;
            }
        }
    });

    let show_more = move |_| {
        let current = view.peek();
        if let Some(visible) = current.visible {
            let step = current
                .filter
                .as_ref()
                .map_or(DEFAULT_LIMIT, |filter| filter.limit);
            let total = current.records.len();
            drop(current);
            view.write().visible = Some((visible + step as usize).min(total));
        } else if let Some((filter, cursor)) = current.filter.clone().zip(current.cursor.clone()) {
            drop(current);
            spawn(read_page(api, session, view, filter, Some(cursor)));
        }
    };

    let cancel = move |_| {
        if let Some(task) = scan_task.take() {
            task.cancel();
            let mut current = view.write();
            current.loading = false;
            current.notice = Some("Loading canceled.".to_owned());
        }
    };

    let current = view.read();
    if current.forbidden {
        return rsx! {
            p { class: "muted", "{NOT_ALLOWED}" }
        };
    }
    let can_show_more = !current.loading
        && match current.visible {
            Some(visible) => visible < current.records.len(),
            None => current.cursor.is_some() && current.has_more,
        };
    let following = current.following;
    let can_follow = current.cursor.is_some() || following;

    rsx! {
        section { class: "change-filters",
            div { class: "filter-row",
                label {
                    "Order"
                    select {
                        aria_label: "Order",
                        disabled: current.loading,
                        onchange: move |event| {
                            let next = if event.value() == Order::NewestFirst.as_str() {
                                Order::NewestFirst
                            } else {
                                Order::OldestFirst
                            };
                            order.set(next);
                            load();
                        },
                        for option_order in Order::ALL {
                            option {
                                value: option_order.as_str(),
                                selected: option_order == order(),
                                "{option_order.label()}"
                            }
                        }
                    }
                }
                label {
                    "Since (UTC)"
                    input {
                        r#type: "datetime-local",
                        value: since,
                        oninput: move |event| since.set(event.value()),
                    }
                }
                label {
                    "Until (UTC)"
                    input {
                        r#type: "datetime-local",
                        value: until,
                        oninput: move |event| until.set(event.value()),
                    }
                }
                label {
                    "Page size"
                    input {
                        r#type: "number",
                        min: "1",
                        max: "{changes::MAX_LIMIT}",
                        value: limit,
                        oninput: move |event| limit.set(event.value()),
                    }
                }
            }
            fieldset { class: "permissions",
                legend { "Groups (none selected means all)" }
                div { class: "flag-grid",
                    for group in MetadataChangeGroup::ALL {
                        label { class: "checkbox", key: "{group.as_str()}",
                            input {
                                r#type: "checkbox",
                                checked: groups.read().contains(&group),
                                onchange: move |event| {
                                    if event.checked() {
                                        groups.write().insert(group);
                                    } else {
                                        groups.write().remove(&group);
                                    }
                                },
                            }
                            {group.as_str().replace('_', " ")}
                        }
                    }
                }
            }
            div { class: "change-actions",
                button { disabled: current.loading, onclick: move |_| load(), "Load" }
                button { class: "secondary", disabled: !can_show_more, onclick: show_more, "Show more" }
                button {
                    class: "secondary",
                    disabled: !can_follow,
                    onclick: move |_| {
                        let mut current = view.write();
                        current.following = !current.following;
                    },
                    if following { "Stop following" } else { "Follow" }
                }
                if scan_task.read().is_some() {
                    button { class: "secondary", onclick: cancel, "Cancel" }
                    span { class: "muted", role: "status",
                        "Loading recent changes, {current.scanned_pages} pages read"
                    }
                }
                if current.loaded {
                    span { class: "muted",
                        "Showing {current.shown().len()} of {current.records.len()} loaded changes"
                    }
                }
            }
            if order() == Order::NewestFirst {
                p { class: "muted",
                    "Showing up to {MAX_SHOWN} recent changes. Choose Oldest first to browse from the beginning."
                }
            }
            if let Some(problem) = &current.problem {
                p { class: "form-error", "{problem}" }
            }
            if let Some(notice) = &current.notice {
                p { class: "muted", role: "status", "{notice}" }
            }
        }
        ChangeTable { view }
    }
}

/// The log's rows. Its own component, so a keystroke in the filters, which
/// re-renders `ChangeLog`, does not diff up to `MAX_SHOWN` rows.
#[component]
fn ChangeTable(view: Signal<LogView>) -> Element {
    let mut selected = use_signal(|| None::<Rc<MetadataChangeRecord>>);
    let current = view.read();
    let shown = current.shown();

    rsx! {
        table {
            thead {
                tr {
                    th { "Operation" }
                    th { "Time" }
                    th { "Kind" }
                    th { "Actor" }
                    th { "Change" }
                }
            }
            tbody {
                if shown.is_empty() {
                    tr {
                        td { colspan: "5", class: "muted",
                            if current.loading { "Loading changes…" } else { "No changes match these filters." }
                        }
                    }
                }
                for record in shown {
                    ChangeRow {
                        key: "{record.op}:{record.ordinal}",
                        record: Rc::clone(record),
                        on_select: move |record| selected.set(Some(record)),
                    }
                }
            }
        }
        if let Some(record) = selected() {
            RecordDetails { record, on_close: move |_| selected.set(None) }
        }
    }
}

/// One row of the log. As its own component it re-renders only when its
/// record changes, so a Follow poll or Show more renders just the new rows.
#[component]
fn ChangeRow(
    record: Rc<MetadataChangeRecord>,
    on_select: EventHandler<Rc<MetadataChangeRecord>>,
) -> Element {
    rsx! {
        tr {
            td {
                span { class: "mono", "{record.op}" }
                span { class: "muted ordinal", "Ordinal {record.ordinal}" }
            }
            td { {format::timestamp(record.timestamp)} }
            td {
                span { class: "badge", {record.kind_label()} }
            }
            td { {actor(&record)} }
            td {
                button {
                    class: "record-button",
                    aria_label: "Inspect {record.kind_label()} at operation {record.op}, ordinal {record.ordinal}",
                    onclick: {
                        let record = Rc::clone(&record);
                        move |_| on_select.call(Rc::clone(&record))
                    },
                    span { {record.summary()} }
                    span { class: "muted", "Details" }
                }
            }
        }
    }
}

/// The record's options with their kind and provenance, when it has any.
struct OptionRow {
    key: String,
    value: String,
    kind: Option<String>,
    explicit: Option<bool>,
}

#[component]
fn RecordDetails(record: Rc<MetadataChangeRecord>, on_close: EventHandler<()>) -> Element {
    let mut feedback = use_signal(String::new);
    let payload = serde_json::to_string_pretty(&*record).unwrap_or_default();
    let options = option_rows(&record);

    let copy = move |text: String, label: String| {
        spawn(async move {
            let message = if copy_to_clipboard(text).await {
                format!("{label} copied")
            } else {
                "Copy is unavailable. You can select and copy the text below.".to_owned()
            };
            feedback.set(message.clone());
            TimeoutFuture::new(COPY_FEEDBACK_MS).await;
            // A newer copy may have replaced the message meanwhile.
            if *feedback.peek() == message {
                feedback.set(String::new());
            }
        });
    };

    rsx! {
        Modal { title: record.kind_label(), on_close, wide: true,
            div { class: "inspector",
                p { class: "muted", {record.summary()} }
                dl { class: "details",
                    dt { "Operation" }
                    dd { class: "mono", "{record.op}" }
                    dt { "Record ordinal" }
                    dd { "{record.ordinal}" }
                    dt { "Actor" }
                    dd { {actor(&record)} }
                    dt { "Time" }
                    dd { {format::timestamp_precise(record.timestamp)} }
                }
                if !options.is_empty() {
                    section {
                        h3 { "Options" }
                        for option in options {
                            div { class: "option-row", key: "{option.key}",
                                div { class: "option-heading",
                                    span { class: "mono", "{option.key}" }
                                    if let Some(kind) = &option.kind {
                                        span { class: "badge", "{kind}" }
                                    }
                                    if let Some(explicit) = option.explicit {
                                        span { class: "badge",
                                            if explicit { "Explicit" } else { "Default / derived" }
                                        }
                                    }
                                    button {
                                        class: "secondary small",
                                        aria_label: "Copy {option.key}",
                                        onclick: {
                                            let value = option.value.clone();
                                            let key = option.key.clone();
                                            move |_| copy(value.clone(), key.clone())
                                        },
                                        "Copy value"
                                    }
                                }
                                pre { "{option.value}" }
                            }
                        }
                    }
                }
                section {
                    h3 { "Full payload" }
                    pre { aria_label: "Full JSON payload", "{payload}" }
                    div { class: "change-actions",
                        button {
                            onclick: {
                                let payload = payload.clone();
                                move |_| copy(payload.clone(), "Payload".to_owned())
                            },
                            "Copy payload"
                        }
                        span { class: "muted", role: "status", "{feedback}" }
                    }
                }
            }
        }
    }
}

/// Builds the filter from the form, or says why the server would refuse it.
fn read_filter(
    since: &str,
    until: &str,
    limit: &str,
    groups: &BTreeSet<MetadataChangeGroup>,
) -> Result<ChangesFilter, FilterError> {
    let filter = ChangesFilter {
        groups: groups.clone(),
        since: changes::micros_from_utc_input(since),
        until: changes::micros_from_utc_input(until),
        limit: limit.trim().parse().unwrap_or(0),
    };
    filter.validate()?;
    Ok(filter)
}

/// Reads one page forward from `after`, or from the earliest change.
async fn read_page(
    api: Api,
    session: Signal<Option<Session>>,
    mut view: Signal<LogView>,
    filter: ChangesFilter,
    after: Option<String>,
) {
    view.write().loading = true;
    let query = filter.query(Start::Earliest, after.as_deref());
    let client = api.client();
    let result = client.metadata_changes(&query).await;
    let mut current = view.write();
    current.loading = false;
    let page = match result {
        Ok(page) => page,
        Err(error) => return fail(&mut current, session, error),
    };
    current.loaded = true;
    current.notice = page.status.message(current.following).map(str::to_owned);
    match page.status {
        MetadataChangeStatus::Ok => {
            current
                .records
                .extend(page.records.into_iter().map(Rc::new));
            let excess = current.records.len().saturating_sub(MAX_SHOWN);
            current.records.drain(..excess);
            current.cursor = page.next;
            current.has_more = page.has_more;
        }
        // The same position is retried, by Follow or by the next Show more.
        MetadataChangeStatus::Behind => {}
        MetadataChangeStatus::Expired | MetadataChangeStatus::Reset => {
            current.cursor = None;
            current.has_more = false;
            current.following = false;
        }
    }
}

/// Loads "newest first" with the scan from `changes::scan_recent`.
async fn scan_newest(
    api: Api,
    session: Signal<Option<Session>>,
    mut view: Signal<LogView>,
    mut scan_task: Signal<Option<Task>>,
    filter: ChangesFilter,
) {
    let client = api.client();
    let mut progress = view;
    let result = changes::scan_recent(
        &filter,
        MAX_SHOWN,
        |query| {
            let client = client.clone();
            async move { client.metadata_changes(&query).await }
        },
        |pages| progress.write().scanned_pages = pages,
    )
    .await;
    scan_task.set(None);
    let mut current = view.write();
    current.loading = false;
    current.loaded = true;
    match result {
        Ok(recent) if recent.page.status == MetadataChangeStatus::Ok => {
            current.records = recent.page.records.into_iter().map(Rc::new).collect();
            current.cursor = recent.page.next;
            if recent.trimmed {
                current.notice = Some(format!(
                    "Showing the newest {MAX_SHOWN} matching records. Narrow the time range or load from the earliest retained change to browse further back."
                ));
            }
        }
        Ok(recent) => {
            current.notice = match recent.page.status {
                MetadataChangeStatus::Behind => Some(
                    "This node is behind the captured position. Load again shortly.".to_owned(),
                ),
                status => status.message(false).map(str::to_owned),
            };
        }
        Err(ScanError::Read(error)) => fail(&mut current, session, error),
        Err(error) => current.problem = Some(error.to_string()),
    }
}

/// Records a failed read. A 403 only hides the log, while a 401 ends the
/// session and the layout returns to sign-in.
fn fail(current: &mut LogView, mut session: Signal<Option<Session>>, error: ApiError) {
    current.following = false;
    match error {
        ApiError::Forbidden => current.forbidden = true,
        ApiError::Unauthorized => {
            let failed = session.write().take();
            if let Some(failed) = failed {
                failed.forget();
            }
        }
        other => current.problem = Some(other.to_string()),
    }
}

fn actor(record: &MetadataChangeRecord) -> String {
    match record.actor_user_id {
        Some(user_id) => format!("user {user_id}"),
        None => "server".to_owned(),
    }
}

fn option_rows(record: &MetadataChangeRecord) -> Vec<OptionRow> {
    let Some(Value::Object(options)) = record.fields.get("options") else {
        return Vec::new();
    };
    options
        .iter()
        .map(|(key, option)| {
            let field = option.as_object();
            let value = field.and_then(|field| field.get("value")).unwrap_or(option);
            OptionRow {
                key: key.clone(),
                value: display_value(value),
                kind: field
                    .and_then(|field| field.get("kind"))
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                explicit: field
                    .and_then(|field| field.get("explicit"))
                    .and_then(Value::as_bool),
            }
        })
        .collect()
}

fn display_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// Copies through the browser's clipboard API, which may refuse, for
/// example on a page served over plain HTTP from another host.
async fn copy_to_clipboard(text: String) -> bool {
    let eval = document::eval(
        "const text = await dioxus.recv(); await navigator.clipboard.writeText(text); return true;",
    );
    eval.send(text).is_ok() && eval.await.is_ok()
}
