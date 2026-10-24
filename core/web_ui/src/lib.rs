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

mod api;
mod auth;
mod components;
mod decode;
mod format;
mod forms;
mod models;
mod pages;
mod permissions;
mod settings;
mod storage;

use dioxus::prelude::*;

use crate::auth::Session;
use crate::components::RefreshTick;
use crate::pages::{
    DashboardLayout, Messages, NotFound, Overview, SignIn, StreamPage, Streams, TopicPage, Users,
    WebUiSettings,
};
use crate::settings::UiSettings;

const STYLESHEET: Asset = asset!("/assets/main.css");
const FAVICON: Asset = asset!("/assets/favicon.svg");
const LOGO_ON_DARK: Asset = asset!("/assets/iggy-apache-darkbg.svg");
const LOGO_ON_LIGHT: Asset = asset!("/assets/iggy-apache-lightbg.svg");

#[derive(Debug, Clone, PartialEq, Routable)]
#[rustfmt::skip]
pub enum Route {
    #[redirect("/", || Route::SignIn {})]
    #[route("/auth/sign-in")]
    SignIn {},
    #[layout(DashboardLayout)]
        #[redirect("/dashboard", || Route::Overview {})]
        #[route("/dashboard/overview")]
        Overview {},
        #[route("/dashboard/streams")]
        Streams {},
        #[route("/dashboard/streams/:stream_id")]
        StreamPage { stream_id: u32 },
        #[route("/dashboard/streams/:stream_id/topics/:topic_id")]
        TopicPage { stream_id: u32, topic_id: u32 },
        #[route("/dashboard/streams/:stream_id/topics/:topic_id/partitions/:partition_id/messages?:offset")]
        Messages { stream_id: u32, topic_id: u32, partition_id: u32, offset: Option<u64> },
        #[route("/dashboard/settings/users")]
        Users {},
        #[route("/dashboard/settings/webUI")]
        WebUiSettings {},
    #[end_layout]
    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}

/// Starts the app in the browser. The binary is a thin wrapper so that tools
/// such as the license manifest, which only follow library targets, see the
/// crate's dependencies.
pub fn launch() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    use_context_provider(|| Signal::new(Session::restore()));
    use_context_provider(|| RefreshTick(Signal::new(0)));
    let settings = use_context_provider(|| Signal::new(UiSettings::restore()));

    rsx! {
        document::Link { rel: "icon", r#type: "image/svg+xml", href: FAVICON }
        document::Stylesheet { href: STYLESHEET }
        div { class: "app", "data-theme": settings.read().theme.as_str(),
            Router::<Route> {}
        }
    }
}
