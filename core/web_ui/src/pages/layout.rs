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
use crate::api::{self, ApiClient};
use crate::components::{CurrentUser, Logo, RefreshTick, use_session};
use crate::models::UserInfoDetails;
use crate::settings::UiSettings;

const IN_FLIGHT_POLL_MS: u32 = 50;

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
            // A tick restarts every page resource, so it waits for the last
            // reload to finish instead of cancelling it and piling up fetches.
            while api::requests_in_flight() > 0 {
                TimeoutFuture::new(IN_FLIGHT_POLL_MS).await;
            }
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
    use_context_provider(|| CurrentUser(user));

    let sign_out = move |_| async move {
        // Logout does not revoke the bearer, so the token stays valid on the
        // server until `expiry` whether or not this call succeeds.
        let client = ApiClient::new(session.peek().as_ref());
        let _ = client.logout().await;
        let signed_out = session.write().take();
        if let Some(signed_out) = signed_out {
            signed_out.forget();
        }
    };

    if session.read().is_none() {
        return rsx! {};
    }

    let can_read_changes = user
        .read()
        .as_ref()
        .and_then(Option::as_ref)
        .is_some_and(UserInfoDetails::can_read_metadata_changes);
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
                    if can_read_changes {
                        Link { class: "nav-link", active_class: "active", to: Route::Changes {}, "Metadata changes" }
                    }
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
