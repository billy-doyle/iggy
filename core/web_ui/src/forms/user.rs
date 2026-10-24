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

use crate::components::{FormActions, Modal, focus_on_mount, use_api};
use crate::models::{CreateUser, ROOT_USER_ID, UserInfo, UserStatus, UserUpdate};
use crate::permissions::PermissionsDraft;

use super::permissions::PermissionsEditor;
use super::{Password, Username, run_request};

#[component]
pub fn NewUserForm(on_close: EventHandler<()>, on_created: EventHandler<()>) -> Element {
    let api = use_api();
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let status = use_signal(|| UserStatus::Active);
    let draft = use_signal(PermissionsDraft::default);
    let busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let submit = move |event: FormEvent| async move {
        event.prevent_default();
        let parsed = username
            .read()
            .parse::<Username>()
            .map_err(|invalid| invalid.to_string())
            .and_then(|username| {
                let password = password
                    .read()
                    .parse::<Password>()
                    .map_err(|invalid| invalid.to_string())?;
                Ok((username, password))
            });
        let (Username(username), Password(password)) = match parsed {
            Ok(parsed) => parsed,
            Err(invalid) => {
                error.set(Some(invalid));
                return;
            }
        };
        let user = CreateUser {
            username,
            password,
            status: status(),
            permissions: draft.read().to_json(),
        };
        let created =
            run_request(api.session(), busy, error, api.client().create_user(&user)).await;
        if created.is_some() {
            on_created.call(());
        }
    };

    rsx! {
        Modal { title: "New user", on_close, busy: busy(), wide: true,
            form { class: "form", onsubmit: submit,
                div { class: "form-row",
                    label {
                        "Username"
                        input {
                            required: true,
                            onmounted: focus_on_mount,
                            autocomplete: "off",
                            value: username,
                            oninput: move |event| username.set(event.value()),
                        }
                    }
                    label {
                        "Password"
                        input {
                            r#type: "password",
                            required: true,
                            autocomplete: "new-password",
                            value: password,
                            oninput: move |event| password.set(event.value()),
                        }
                    }
                    StatusSelect { status }
                }
                PermissionsEditor { draft }
                FormActions {
                    submit_label: "Create",
                    busy: busy(),
                    error: error(),
                    on_cancel: on_close,
                }
            }
        }
    }
}

#[component]
pub fn EditUserForm(
    user: UserInfo,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let api = use_api();
    let user_id = user.id;
    // Taken once at mount, so a refreshed `user` cannot make an untouched
    // field look edited and revert someone else's change.
    let initial_username = use_signal(|| user.username.clone());
    let initial_status = use_signal(|| user.status);
    let mut username = use_signal(|| user.username.clone());
    let status = use_signal(|| user.status);
    let busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);

    let submit = move |event: FormEvent| async move {
        event.prevent_default();
        let Username(next_username) = match username.read().parse::<Username>() {
            Ok(username) => username,
            Err(invalid) => {
                error.set(Some(invalid.to_string()));
                return;
            }
        };
        let update = UserUpdate {
            username: (next_username != *initial_username.read()).then_some(next_username),
            status: (status() != initial_status()).then_some(status()),
        };
        if update == UserUpdate::default() {
            on_close.call(());
            return;
        }
        let saved = run_request(
            api.session(),
            busy,
            error,
            api.client().update_user(user_id, &update),
        )
        .await;
        if saved.is_some() {
            on_saved.call(());
        }
    };

    rsx! {
        Modal { title: "Edit user {user.username}", on_close, busy: busy(),
            form { class: "form", onsubmit: submit,
                label {
                    "Username"
                    input {
                        required: true,
                        onmounted: focus_on_mount,
                        value: username,
                        oninput: move |event| username.set(event.value()),
                    }
                }
                if user_id != ROOT_USER_ID {
                    StatusSelect { status }
                }
                FormActions {
                    submit_label: "Save",
                    busy: busy(),
                    error: error(),
                    on_cancel: on_close,
                }
            }
        }
    }
}

#[component]
fn StatusSelect(status: Signal<UserStatus>) -> Element {
    rsx! {
        label {
            "Status"
            select {
                onchange: move |event| {
                    if let Ok(next) = event.value().parse() {
                        status.set(next);
                    }
                },
                for option_status in UserStatus::ALL {
                    option {
                        value: option_status.as_str(),
                        selected: option_status == status(),
                        "{option_status.label()}"
                    }
                }
            }
        }
    }
}
