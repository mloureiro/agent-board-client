# agent-board-client

A desktop client for [agent-board](https://github.com/mloureiro/agent-board) servers, built with
Tauri 2. It keeps a list of servers and opens each one in its own window, using the operating
system's webview.

The client is becoming a full board UI of its own. Until every view is ported, pages it does not
draw yet still open the board's own page, which may ask you to sign in and unlock separately.

- A server is a name and an address. `https` is required; plain `http` is accepted only for
  `localhost`, `127.0.0.1` and `[::1]`.
- Each server gets its own persistent website data store, so sign-in cookies and the board's keys
  survive restarts and are never shared between servers.
- The client stores the server list and, per server, a tab file with the path and label of each
  open tab. A label can be a decrypted task title, saved in plaintext. The tab file is deleted when
  the server is removed.
- It will also store, per server, a session token in the system keychain and the keys derived from
  your passphrase in that server's own data store. It never stores the passphrase or the data key.
- All requests to a board will go from the app's native side. The client's own pages never call a
  board directly.
- A board page cannot call the client's native commands. Only the server list window can, and the
  tab bar of each server window can call three tab commands (snapshot, select, close) for its own
  window only. The client's own pages (`board.html`) can call a fixed set of commands that act for
  their own server only. None exists yet.

macOS 14 or newer is required. Other platforms compile but are untested.

## Signing in

Passkeys do not work inside a server window, because the system webview refuses them for this
app. A board that offers "Sign in with your browser" shows a code and a link in the window. The
link opens in the default browser, where you sign in by any method and approve after checking
that the code matches. The window then signs in by itself.

The board must run a version with this hand-off flow. Other sign-in methods keep working in the
window.

## Development

Requires Node 24 or newer, pnpm, and a stable Rust toolchain with `rustfmt` and `clippy`.

```sh
pnpm install
pnpm dev            # run the app against the Vite dev server
pnpm build          # build an unsigned app bundle into src-tauri/target/release/bundle
```

`@agent-board/shared` is a git dependency on the public fork, pinned to a commit. No token is
needed to install it.

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
- `board.html`, `src/board/`: the client's own board app (React and Mantine), shown in a tab instead
  of the board's page once a page is ported.
- `fixtures/native-pages.json`: the ported pages, empty until the first view lands. Listing `home`
  also moves notifications and the health banner off the board page.
- `fixtures/tab-kind-cases.json`: which pages open as native or board tabs, run by both test suites.
- `tooling/`: build checks, including the one that keeps React out of the server list and tab bar.
- `tabbar.html`, `src/tabbar*.ts`: the tab bar webview at the top of each server window.
- `src-tauri/scripts/`: scripts injected into board pages, one reporting navigation to the client
  and one handling external links.
- `src-tauri/`: the Rust core, with the server list file, the address rule and one window per
  server.
- `fixtures/server-url-cases.json`: the address rule as a case table, run by both test suites.
- `fixtures/page-cases.json`: the page-path rule as a case table, run by both test suites.
