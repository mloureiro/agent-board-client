# agent-board-client

A desktop client for [agent-board](https://github.com/mloureiro/agent-board) servers, built with
Tauri 2. It keeps a list of servers and opens each one in its own window, using the operating
system's webview.

- A server is a name and an address. `https` is required; plain `http` is accepted only for
  `localhost`, `127.0.0.1` and `[::1]`.
- Each server gets its own persistent website data store, so sign-in cookies and the board's keys
  survive restarts and are never shared between servers.
- The client stores the server list and nothing else: no token, key or password.
- A board page can call one native command: the one that asks the client to show a notification.
  Every other command is open to the server list window only.
- Notifications are shown by the client, never by a server. Each server has a switch in the list.
  A board page that creates a `Notification` reaches the client, which shows it under the server's
  name when that server's switch is on.
- While a server's switch is on, its page stays loaded after its window closes, so it can keep
  reporting. The app stays in the menu bar until you choose Quit there.

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

## Layout

- `src/`: the server list window (TypeScript, no framework).
- `src/board-notifications.js`: the `Notification` every board page gets, injected by the Rust core.
- `src-tauri/`: the Rust core, with the server list file, the address rule, one window per server,
  the notification command and the menu-bar icon.
- `fixtures/server-url-cases.json`: the address rule as a case table, run by both test suites.
