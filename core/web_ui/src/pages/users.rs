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
use crate::components::{ErrorView, Loading, PageHeader, api_client, use_session};
use crate::format;
use crate::forms::{EditUserForm, NewUserForm};
use crate::models::{ROOT_USER_ID, UserInfo, UserStatus};

#[derive(Debug, Clone, Copy, PartialEq)]
enum UserDialog {
    New,
    Edit(u32),
}

#[component]
pub fn Users() -> Element {
    let mut session = use_session();
    let mut search = use_signal(String::new);
    let mut dialog = use_signal(|| None::<UserDialog>);
    let mut users = use_resource(move || async move {
        let mut users = api_client(session).users().await?;
        users.sort_by_key(|user| user.id);
        Ok::<_, ApiError>(users)
    });
    let close = move |_| dialog.set(None);
    let saved = move |_| {
        let edited_self = matches!(
            (dialog(), session.read().as_ref()),
            (Some(UserDialog::Edit(user_id)), Some(signed_in)) if signed_in.user_id == user_id
        );
        dialog.set(None);
        users.restart();
        // The sidebar's username comes from a resource that depends only on
        // the session, so rewriting the session reloads it after a self-rename.
        if edited_self {
            session.write();
        }
    };

    let body = match &*users.read() {
        None => rsx! { Loading {} },
        Some(Err(error)) => rsx! { ErrorView { error: error.clone() } },
        Some(Ok(all_users)) => {
            let query = search.read().trim().to_lowercase();
            let matching: Vec<&UserInfo> = all_users
                .iter()
                .filter(|user| query.is_empty() || user.username.to_lowercase().contains(&query))
                .collect();
            rsx! {
                p { class: "muted", "{matching.len()} of {all_users.len()} users" }
                table {
                    thead {
                        tr {
                            th { "ID" }
                            th { "Username" }
                            th { "Status" }
                            th { "Created" }
                            th {}
                        }
                    }
                    tbody {
                        for user in matching {
                            tr { key: "{user.id}",
                                td { "{user.id}" }
                                td {
                                    "{user.username}"
                                    if user.id == ROOT_USER_ID {
                                        span { class: "badge", "root" }
                                    }
                                }
                                td {
                                    span {
                                        class: if user.status == UserStatus::Active { "badge active" } else { "badge" },
                                        "{user.status.as_str()}"
                                    }
                                }
                                td { {format::timestamp(user.created_at)} }
                                td { class: "num",
                                    button {
                                        class: "secondary small",
                                        onclick: {
                                            let user_id = user.id;
                                            move |_| dialog.set(Some(UserDialog::Edit(user_id)))
                                        },
                                        "Edit"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    rsx! {
        PageHeader {
            title: "Users",
            actions: rsx! {
                input {
                    class: "search",
                    r#type: "search",
                    aria_label: "Search users",
                    placeholder: "Search",
                    value: search,
                    oninput: move |event| search.set(event.value()),
                }
                button { onclick: move |_| dialog.set(Some(UserDialog::New)), "New user" }
            },
        }
        {body}
        match dialog() {
            Some(UserDialog::New) => rsx! {
                NewUserForm { on_close: close, on_created: saved }
            },
            Some(UserDialog::Edit(user_id)) => {
                let user = users
                    .read()
                    .as_ref()
                    .and_then(|users| users.as_ref().ok()?.iter().find(|user| user.id == user_id).cloned());
                match user {
                    Some(user) => rsx! {
                        EditUserForm { user, on_close: close, on_saved: saved }
                    },
                    None => rsx! {},
                }
            }
            None => rsx! {},
        }
    }
}
