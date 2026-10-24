# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

# Standalone web UI image, published as apache/iggy-web-ui. The Helm chart
# runs it next to the server, so it keeps the Svelte image's contract: port
# 3050 (PORT), GET /healthz, and the API URL from PUBLIC_IGGY_API_URL.
#
# Build context is the repository root, like the other images.

ARG RUST_VERSION=1.99.1
ARG NGINX_VERSION=1.30.6

# The bundle is WebAssembly, the same on every platform, so it is built once
# on the build machine. dx only ships glibc Linux builds, hence Debian.
FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION}-slim-trixie AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
COPY . .
RUN ./scripts/ci/install-dx.sh /usr/local/bin && ./scripts/build-web-ui.sh
# Required by ASF release policy: convenience binaries that bundle
# third-party code must ship the license text of every bundled crate.
RUN TARGET="$(uname -m)-unknown-linux-musl" && \
    curl -sSfL "https://github.com/EmbarkStudios/cargo-about/releases/download/0.9.0/cargo-about-0.9.0-${TARGET}.tar.gz" \
      | tar -xz -C /usr/local/bin --strip-components=1 "cargo-about-0.9.0-${TARGET}/cargo-about" && \
    ./scripts/ci/third-party-licenses.sh --generate \
      --manifest core/web_ui/Cargo.toml \
      --output /build/LICENSE-binary

FROM nginxinc/nginx-unprivileged:${NGINX_VERSION}-alpine
ENV PORT=3050
COPY core/web_ui/docker/default.conf.template /etc/nginx/templates/default.conf.template
COPY --chmod=755 core/web_ui/docker/40-iggy-web-ui-config.sh /docker-entrypoint.d/40-iggy-web-ui-config.sh
# Owned by the nginx user so the start-up script can rewrite config.json.
COPY --from=builder --chown=101:101 /build/core/web_ui/dist/public /usr/share/nginx/html/ui
COPY --from=builder /build/LICENSE-binary /usr/share/doc/iggy-web-ui/LICENSE-binary
COPY LICENSE NOTICE /usr/share/doc/iggy-web-ui/
EXPOSE 3050
