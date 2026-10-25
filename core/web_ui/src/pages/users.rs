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
use crate::components::{ErrorView, Loading, PageHeader, use_api};
use crate::format;
use crate::forms::{EditUserForm, NewUserForm};
use crate::models::{ROOT_USER_ID, UserInfo, UserStatus};

#[derive(Debug, Clone, PartialEq)]
enum UserDialog {
    New,
    Edit(UserInfo),
}

#[component]
pub fn Users() -> Element {
    let api = use_api();
    let mut search = use_signal(String::new);
    let mut dialog = use_signal(|| None::<UserDialog>);
    let mut users = use_resource(move || async move {
        let mut users = api.client().users().await?;
        users.sort_by_key(|user| user.id);
        Ok::<_, ApiError>(users)
    });
    let close = move |_| dialog.set(None);
    let saved = move |_| {
        dialog.set(None);
        users.restart();
    };

    let body = match &*users.read() {
        None => rsx! { Loading {} },
        Some(Err(error)) => rsx! { ErrorView { error: error.clone() } },
        Some(Ok(all_users)) => {
            let query = search().trim().to_lowercase();
            let matching: Vec<&UserInfo> = all_users
                .iter()
                .filter(|user| user.username.to_lowercase().contains(&query))
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
                                            let user = user.clone();
                                            move |_| dialog.set(Some(UserDialog::Edit(user.clone())))
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
            Some(UserDialog::Edit(user)) => rsx! {
                EditUserForm { user, on_close: close, on_saved: saved }
            },
            None => rsx! {},
        }
    }
}
