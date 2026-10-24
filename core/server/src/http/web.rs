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

use axum::Router;
use axum::body::Body;
use axum::extract::Path;
use axum::http::{Response, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use rust_embed::{Embed, EmbeddedFile};

#[derive(Embed)]
#[folder = "../web_ui/dist/public/"]
#[allow_missing = true]
struct WebAssets;

impl WebAssets {
    fn get_file(path: &str) -> Option<EmbeddedFile> {
        <Self as Embed>::get(path)
    }
}

pub fn router() -> Router {
    Router::new()
        .route("/ui/{*wildcard}", get(serve_web_asset))
        .route("/ui", get(serve_index))
        .route("/ui/", get(serve_index))
}

async fn serve_index() -> impl IntoResponse {
    serve_file("index.html")
}

async fn serve_web_asset(Path(wildcard): Path<String>) -> impl IntoResponse {
    if let Some(response) = try_serve_file(&wildcard) {
        return response;
    }

    if !wildcard.contains('.') {
        return serve_file("index.html");
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))
        .unwrap()
}

fn try_serve_file(path: &str) -> Option<Response<Body>> {
    let asset = WebAssets::get_file(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();

    Some(
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, cache_control(path))
            .body(Body::from(asset.data.into_owned()))
            .unwrap(),
    )
}

/// File names under `assets/` carry a content hash, so a cached copy never
/// goes stale. `index.html` and `config.json` are checked on every load.
fn cache_control(path: &str) -> &'static str {
    if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

fn serve_file(path: &str) -> Response<Body> {
    try_serve_file(path).unwrap_or_else(|| {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not Found"))
            .unwrap()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_hashed_asset_should_be_cached_as_immutable() {
        assert_eq!(
            cache_control("assets/iggy-web-ui_bg-dxh8db675f38535abb6.wasm"),
            "public, max-age=31536000, immutable"
        );
    }

    #[test]
    fn given_app_shell_or_config_should_be_revalidated() {
        assert_eq!(cache_control("index.html"), "no-cache");
        assert_eq!(cache_control("config.json"), "no-cache");
    }
}
