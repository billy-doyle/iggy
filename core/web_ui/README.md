# Apache Iggy Web UI

A [Dioxus](https://dioxuslabs.com/) single-page app for browsing an Iggy
server over its HTTP API. It compiles to `wasm32-unknown-unknown` and is
built with the Dioxus CLI, `dx`.

## Prerequisites

`dx` must match the `dioxus` version in `Cargo.lock`. `scripts/ci/install-dx.sh`
pins it as `DX_VERSION` and checks it against `Cargo.lock`. On Linux (glibc)
and macOS it downloads that build into `~/.cargo/bin`, or the directory passed
as its argument. Elsewhere it prints the `cargo install` command to run.
On Linux it also installs the wasm-bindgen, wasm-opt and esbuild that
`dx bundle` would otherwise download, so CI can build with `NO_DOWNLOADS=1`.

```bash
./scripts/ci/install-dx.sh
rustup target add wasm32-unknown-unknown
```

## Release bundle

```bash
./scripts/build-web-ui.sh
```

The bundle lands in `core/web_ui/dist`, with every asset path under `/ui`.
`iggy-server` embeds it from there at compile time, so build the bundle first
or the server has no UI to serve.
The script clears dx's staging directory first, because dx copies every
previously hashed asset into the bundle.

## Development

Start a server, point the UI at it, and serve the UI with hot reload:

```bash
cargo run --bin iggy-server -- --with-default-root-credentials
cd core/web_ui
echo '{"api_url": "http://localhost:3000"}' > public/config.json
dx serve
```

Open <http://localhost:8080/ui/> and sign in as `iggy` / `iggy`. Restore
`public/config.json` before committing.

The UI reads `config.json` from the bundle root before its first API call,
and `api_url` prefixes every request. The committed file leaves it empty, so
requests go to the origin that served the UI. The server's default CORS
settings allow any origin, so the dev server on another port can reach it.

## Standalone image

```bash
docker build -f core/web_ui/Dockerfile -t apache/iggy-web-ui .
docker run -p 3050:3050 -e PUBLIC_IGGY_API_URL=http://localhost:3000 apache/iggy-web-ui
```

nginx serves the bundle under `/ui` on `PORT` (3050) with a `/healthz`
check. The start-up script writes `PUBLIC_IGGY_API_URL` into `config.json`.

## Tests

```bash
cargo test -p iggy-web-ui
```

The tests run on the host. Besides the formatting helpers, they serialize
`iggy_common` types and decode them into the UI's mirrors in `src/models.rs`,
so a renamed server field fails here instead of in the browser.
