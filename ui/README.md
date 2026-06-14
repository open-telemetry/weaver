# Weaver UI

The web UI served by `weaver serve`. It's a [React](https://react.dev/) +
[TypeScript](https://www.typescriptlang.org/) single-page app built with
[Vite](https://vite.dev/), routed by
[TanStack Router](https://tanstack.com/router), and styled with
[Tailwind CSS](https://tailwindcss.com/) + [daisyUI](https://daisyui.com/).

At release time the production build (`ui/dist`) is embedded directly into the
`weaver` binary (see `src/serve/ui.rs`), so the running server serves the UI and
its `/api/v1/*` endpoints from the same origin.

## Prerequisites

- **Node.js** — version pinned in [`.nvmrc`](../.nvmrc) (use `nvm use` from this
  directory).
- **pnpm** — version pinned via the `packageManager` field in `package.json`
  (enable with `corepack enable`).
- **Rust toolchain** — only needed to run the backend (`weaver serve`).

```sh
pnpm install
```

## Development

Run the backend and the Vite dev server side by side. The dev server proxies
`/api` to `http://localhost:8080` (see `vite.config.ts`), so the backend must be
running:

```sh
# Terminal 1 — backend API (any registry; the bundled live_check model is handy)
cargo run -- serve -r crates/weaver_live_check/model

# Terminal 2 — hot-reloading UI dev server (prints its local URL, e.g. :5173)
pnpm dev
```

## Production build

```sh
pnpm build      # outputs to ui/dist, which is embedded into the weaver binary
pnpm preview    # serve the built assets locally to sanity-check
```

`cargo run -- serve` also rebuilds-check the UI via `build.rs` and warns if
`ui/dist` is stale.

## Linting

```sh
pnpm lint
```

## End-to-end smoke tests

[Playwright](https://playwright.dev/) tests in [`e2e/`](./e2e) load the UI and
exercise a few core flows (search renders, click into a detail page, stats page
counts and links). They're intentionally minimal — a foundational signal that
the UI is built and interactive.

### One-time setup

```sh
pnpm install
pnpm exec playwright install chromium       # browser only
# or: pnpm test:e2e:install                  # browser + OS dependencies (CI)
```

### Running

```sh
pnpm test:e2e
```

How the server is provided depends on what's already running:

- **Nothing on `:8080`** — Playwright's `webServer` (in `playwright.config.ts`)
  boots `cargo run -- serve -r crates/weaver_live_check/model` and waits for
  `/health`. The first run may compile, so allow some time.
- **A server is already on `:8080`** — it's detected and reused (locally only),
  which makes the suite run in ~1s.

Target an externally managed server (e.g. a Docker container) and skip the
auto-start:

```sh
WEAVER_EXTERNAL_SERVER=1 WEAVER_BASE_URL=http://127.0.0.1:8080 pnpm test:e2e
```

| Env var                 | Default                  | Purpose                                          |
| ----------------------- | ------------------------ | ------------------------------------------------ |
| `WEAVER_BASE_URL`       | `http://127.0.0.1:8080`  | URL of the UI under test.                         |
| `WEAVER_EXTERNAL_SERVER`| _(unset)_                | When set, don't auto-start a server.              |

### Watching / debugging

The suite finishes in about a second, so `--headed` just flashes by. To actually
watch it:

```sh
pnpm exec playwright test --ui        # UI mode: step through, time-travel, replay
pnpm exec playwright test --debug     # Playwright Inspector: pause on each action
pnpm exec playwright show-report      # open the HTML report from the last run
```

To slow a headed run down, add `launchOptions: { slowMo: 1000 }` under `use` in
`playwright.config.ts` (don't commit it — it would slow CI).

## CI

- **`ci.yml`** (`ui-smoke` job) builds the UI and binary, then runs the suite
  against `cargo run -- serve`.
- **`publish-docker.yml`** starts the freshly built `otel/weaver` container with
  the live_check model mounted and runs the same suite with
  `WEAVER_EXTERNAL_SERVER=1`.
