# agent-board-client

A desktop client for [agent-board](https://github.com/mloureiro/agent-board) servers, built with
Tauri 2. It keeps a list of servers and opens each one in its own window, using the operating
system's webview.

- A server is a name and an address. `https` is required; plain `http` is accepted only for
  `localhost`, `127.0.0.1` and `[::1]`.
- Each server gets its own persistent website data store, so sign-in cookies and the board's keys
  survive restarts and are never shared between servers.
- The client stores the server list and nothing else: no token, key or password.
- A board page cannot call the client's native commands. Only the server list window can.

macOS 14 or newer is required. Other platforms compile but are untested.

## Development

Requires Node 24 or newer, pnpm, and a stable Rust toolchain with `rustfmt` and `clippy`.

```sh
pnpm install
pnpm dev            # run the app against the Vite dev server
pnpm build          # build an unsigned app bundle into src-tauri/target/release/bundle
```

## Checks

Each script covers the web side and the Rust side.

```sh
pnpm format:check   # prettier, cargo fmt
pnpm lint           # eslint, cargo clippy
pnpm typecheck      # tsc, cargo check
pnpm test           # vitest, cargo test
```

`pnpm format` rewrites files in place.

## Releases

Pushing a tag `v<version>`, matching `version` in `src-tauri/tauri.conf.json`, runs
`.github/workflows/release.yml`. It builds a universal macOS app, ad-hoc signed, in release and
debug (the debug build has the Web Inspector), and attaches both as zips to a GitHub release.
Running the workflow by hand builds the same zips as a run artifact, with no release.

```sh
gh release download v0.1.0 --repo mloureiro/agent-board-client --pattern '*.zip'
ditto -x -k Agent-Board-0.1.0-macos-universal.zip .
```

A file downloaded with `gh` carries no quarantine flag. A zip downloaded in a browser does, and
the unsigned app is then blocked; clear it with `xattr -dr com.apple.quarantine "Agent Board.app"`.

## Layout

- `src/`: the server list window (TypeScript, no framework).
- `src-tauri/`: the Rust core, with the server list file, the address rule and one window per
  server.
- `fixtures/server-url-cases.json`: the address rule as a case table, run by both test suites.
