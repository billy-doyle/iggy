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

//! Settings the hosting environment supplies when the UI starts.
//!
//! The bundle ships a `config.json` with an empty `api_url`, so the UI that
//! `iggy-server` embeds calls its own origin. The standalone image rewrites
//! the file from `PUBLIC_IGGY_API_URL` when the container starts.

use gloo_net::http::Request;
use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default)]
    pub api_url: String,
}

impl RuntimeConfig {
    /// A missing or unreadable file falls back to the server's own origin, so
    /// a hand-built bundle still works when served by `iggy-server`.
    pub async fn load() -> Self {
        let url = config_url(dioxus::cli_config::base_path().as_deref());
        match Request::get(&url).send().await {
            Ok(response) if response.ok() => response.json().await.unwrap_or_default(),
            _ => Self::default(),
        }
    }
}

/// `config.json` sits at the bundle root, which is not the current page's
/// directory once the router has navigated.
fn config_url(base_path: Option<&str>) -> String {
    match base_path.map(|base| base.trim_matches('/')) {
        Some(base) if !base.is_empty() => format!("/{base}/config.json"),
        _ => "/config.json".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_base_path_in_any_form_should_build_root_relative_url() {
        assert_eq!(config_url(Some("ui")), "/ui/config.json");
        assert_eq!(config_url(Some("/ui/")), "/ui/config.json");
        assert_eq!(config_url(Some("")), "/config.json");
        assert_eq!(config_url(None), "/config.json");
    }

    #[test]
    fn given_config_without_api_url_should_default_to_same_origin() {
        let config: RuntimeConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config.api_url, "");
        let config: RuntimeConfig =
            serde_json::from_str(r#"{"api_url":"https://iggy.example.com:3000"}"#).unwrap();
        assert_eq!(config.api_url, "https://iggy.example.com:3000");
    }
}
