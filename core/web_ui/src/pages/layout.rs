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

use gloo_timers::future::TimeoutFuture;

use crate::Route;
use crate::api::ApiClient;
use crate::auth::Session;
use crate::components::{Logo, RefreshTick, use_session};
use crate::settings::UiSettings;

/// Shell for every signed-in page. Without a session it redirects to sign-in.
#[component]
pub fn DashboardLayout() -> Element {
    let mut session = use_session();
    let navigator = navigator();
    let settings = use_context::<Signal<UiSettings>>();
    let RefreshTick(mut refresh) = use_context();

    // A resource rather than a future because reading the interval subscribes
    // it: saving a new interval cancels the current wait instead of letting
    // an hour-long one run out first. The read is its own statement, since a
    // guard held across the await would keep the settings borrowed.
    let _ticker = use_resource(move || async move {
        let interval_ms = settings.read().refresh_interval_ms;
        loop {
            TimeoutFuture::new(interval_ms).await;
            refresh += 1;
        }
    });

    use_effect(move || {
        if session.read().is_none() {
            navigator.replace(Route::SignIn {});
        }
    });

    let user = use_resource(move || async move {
        let user_id = session.read().as_ref()?.user_id;
        let client = ApiClient::new(session.read().as_ref());
        client.user(user_id).await.ok()
    });

    let sign_out = move |_| async move {
        // The token is dropped locally either way, so a failed revoke only
        // leaves it valid on the server until it expires.
        let client = ApiClient::new(session.peek().as_ref());
        let _ = client.logout().await;
        Session::clear();
        session.set(None);
    };

    if session.read().is_none() {
        return rsx! {};
    }

    let username = user
        .read()
        .as_ref()
        .and_then(|user| user.as_ref().map(|user| user.username.clone()))
        .unwrap_or_default();

    rsx! {
        div { class: "dashboard",
            aside { class: "sidebar",
                Logo { class: "sidebar-logo" }
                nav {
                    Link { class: "nav-link", active_class: "active", to: Route::Overview {}, "Overview" }
                    Link { class: "nav-link", active_class: "active", to: Route::Streams {}, "Streams" }
                    Link { class: "nav-link", active_class: "active", to: Route::Users {}, "Users" }
                    Link { class: "nav-link", active_class: "active", to: Route::WebUiSettings {}, "Settings" }
                }
                div { class: "sidebar-footer",
                    button {
                        class: "secondary",
                        title: "Reload this page's data now",
                        onclick: move |_| refresh += 1,
                        "Refresh"
                    }
                    span { class: "muted", "{username}" }
                    button { class: "secondary", onclick: sign_out, "Sign out" }
                }
            }
            main { class: "content", Outlet::<Route> {} }
        }
    }
}
