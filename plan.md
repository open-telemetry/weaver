# UI Migration Backlog — Svelte (`ui/`) → React (`ui-react/`)

## Goal
Create a **separate** React app in `ui-react/` (Vite + React + TypeScript) using **TanStack Router (file-based routing)**, keeping **Tailwind + DaisyUI** and **marked**. Maintain **strict parity** with the existing Svelte UI in `ui/` until cutover.

## Constraints (explicit)
- Separate applications: keep `ui/` and `ui-react/` independent.
- Use **npm**.
- Use **Vite React + TS default tsconfig**.
- Use **TanStack Router** (default behavior).
- No state management library; use React hooks/state.
- Keep current theme strategy: `data-theme` on `<html>` + `localStorage` key `theme`.
- Keep **marked** for Markdown, same behavior as Svelte.
- Keep **RapiDoc** with **CDN script** loading.
- Keep API base path: `/api/v1` (same as Svelte).
- No “fallbacks”; modern browsers only.
- For now, **do not modify** the Rust server UI embed (`src/serve/ui.rs` currently embeds `ui/dist`).

## Backlog (chronological, small/isolated tasks)

### 0) Baseline inventory (Svelte is source-of-truth)
- [x] Confirm current Svelte build output: `ui/dist` (Vite build)
  - `ui/vite.config.js` sets `build.outDir = "dist"`
  - `ui/package.json` uses `vite build` (no custom outDir override)
- [x] List Svelte routes from `ui/src/App.svelte`
  - `/`
  - `/search`
  - `/stats`
  - `/attribute/*`
  - `/metric/*`
  - `/span/*`
  - `/event/*`
  - `/entity/*`
  - `/schema`
  - `/api-docs`
- [x] Record Svelte dev proxy behavior from `ui/vite.config.js` (`/api` → `http://localhost:8080`)
  - `server.proxy` maps `/api` to `http://localhost:8080`
- [x] Record API endpoints used by UI from `ui/src/lib/api.js` (paths + params)
  - `/api/v1/registry/stats`
  - `/api/v1/registry/attribute/:key`
  - `/api/v1/registry/metric/:name`
  - `/api/v1/registry/span/:type`
  - `/api/v1/registry/event/:name`
  - `/api/v1/registry/entity/:type`
  - `/api/v1/registry/search` params: `q`, `type` (omit when `all`), `stability`, `limit`, `offset`
- [x] Record theme behavior from `ui/src/App.svelte` (localStorage key + `data-theme`)
  - Loads `localStorage.getItem("theme") || "light"` on mount
  - Sets `document.documentElement` `data-theme` to saved theme
  - Toggle switches `light` ↔ `dark`, persists to `localStorage` key `theme`, and updates `data-theme`
- [x] Record Markdown behavior from `ui/src/components/Markdown.svelte` (block)
  - Uses `marked.setOptions({ breaks: true, gfm: true })`, renders `marked(...)` synchronously into `<div class="prose prose-sm max-w-none">` with global styles for `a`, `code`, `pre`, `ul`, `ol`, `li`, `p`.
- [x] Record InlineMarkdown behavior from `ui/src/components/InlineMarkdown.svelte` (strip `<p>`)
  - Uses `marked.parse` with `async: false`, `breaks: false`, `gfm: true`, strips `<p>` tags via `/<\/?p>/g`, renders inside a `<span>` with inline styles for `code`, `a`, `strong`, `em`.
- [x] Record Pagination behavior from `ui/src/components/Pagination.svelte` (window size, ellipses)
  - `totalPages = ceil(total / limit)`; `currentPage = floor(offset / limit) + 1`
  - Visible window size `maxVisible = 7` (sliding window centered on current page)
  - Adjusts window when near end (`start` shifts so `end` hits `totalPages`)
  - Shows first/last page buttons with `...` when gaps > 1
  - Prev/next buttons disabled at bounds; `goToPage(page)` sets `offset = (page - 1) * limit`
  - Pagination renders only when `totalPages > 1`
- [x] Record API docs behavior from `ui/src/routes/ApiDocs.svelte` (RapiDoc + theme sync)
  - Loads RapiDoc CDN script `https://unpkg.com/rapidoc/dist/rapidoc-min.js`
  - `spec-url="/api/v1/openapi.json"`, `render-style="read"`, `layout="column"`, `schema-style="tree"`
  - Disables header/auth/server selection; allows try (`allow-try="true"`)
  - Theme sync via `MutationObserver` on `data-theme`, updating RapiDoc color attributes
  - Container is fixed under navbar/sidebar with responsive left offset
- [x] Record Schema page URL behavior from `ui/src/routes/Schema.svelte` (`schema` + `type` query params)
  - Reads `schema` from hash query string (`window.location.hash`), default `ForgeRegistryV2`
  - Fetches `/api/v1/schema/:schema` on load and on `hashchange`
  - Reads `type` from hash query string; `root` or missing shows root, otherwise selects definition
  - `selectDefinition`/`selectRoot` update `type` via `history.pushState` on current URL

### 1) Scaffold `ui-react/` (Vite + React + TS)
- [x] Create `ui-react/` via Vite React + TS template
- [x] Set `ui-react/package.json` name (distinct from `ui/`)
- [x] Ensure `ui-react/` has independent lockfile (`ui-react/package-lock.json`)
- [x] Configure `ui-react` build output directory to `ui-react/dist`
- [x] Add `ui-react` scripts: `dev`, `build`, `preview` (Vite defaults)
- [x] Ensure `ui-react/index.html` contains app mount element (React root)
- [x] Ensure `ui-react/src/main.tsx` imports global CSS

### 2) Tailwind + DaisyUI (mirror Svelte)
- [x] Install Tailwind CSS v4 + @tailwindcss/vite plugin in `ui-react/`
- [x] Install DaisyUI 5.5.14 in `ui-react/` (latest version with @plugin directive)
- [x] Use `@import "tailwindcss"` and `@plugin "daisyui"` in CSS (v4 approach)
- [x] Port global CSS from `ui/src/app.css` into React global CSS (Tailwind layers + custom classes)

**Note:** Upgraded to Tailwind CSS v4 using official Vite plugin approach per https://tailwindcss.com/docs/installation/using-vite. Removed postcss.config.js and tailwind.config.js (not needed with v4).

### 3) Router: TanStack Router (file-based)
- [x] Install TanStack Router (React) packages in `ui-react/`
- [x] Set up TanStack Router file-based routing per docs
- [x] Add route for `/` (root)
- [x] Add route for `/search`
- [x] Add route for `/stats`
- [x] Add route for `/schema`
- [x] Add route for `/api-docs`
- [x] Add route for `/attribute/$key`
- [x] Add route for `/metric/$name`
- [x] Add route for `/span/$type`
- [x] Add route for `/event/$name`
- [x] Add route for `/entity/$type`
- [x] Verify deep links work in dev for all routes

### 4) Dev proxy (same as Svelte)
- [x] Create `ui-react/vite.config.ts`
- [x] Configure dev server proxy: `/api` → `http://localhost:8080`
- [x] Confirm `/api/v1/...` requests succeed via proxy in dev

### 5) App shell layout (DaisyUI parity)
- [x] Recreate Drawer layout (desktop open, mobile toggle) matching `ui/src/App.svelte`
- [x] Recreate Navbar (sticky top) matching `ui/src/App.svelte`
- [x] Recreate Sidebar sections + links (Registry / Schema / Developer)
- [x] Implement "active link" styling parity (based on current path)
- [x] Ensure sidebar links match Svelte destinations exactly

### 6) Theme toggle (exact parity)
- [x] Implement `theme` state (React hooks)
- [x] On load: read `localStorage.getItem('theme') ?? 'light'`
- [x] On theme change: set `document.documentElement.setAttribute('data-theme', theme)`
- [x] Persist theme to `localStorage` under key `theme`
- [x] Implement toggle button + icons matching Svelte behavior

### 7) API client (`/api/v1`, TypeScript)
- [x] Create `ui-react/src/lib/api.ts`
- [x] Implement `BASE_URL = '/api/v1'`
- [x] Implement `fetchJSON()` with same error semantics as Svelte (throw on `!ok`)
- [x] Implement `getRegistryStats()`
- [x] Implement `getAttribute(key)`
- [x] Implement `getMetric(name)`
- [x] Implement `getSpan(type)`
- [x] Implement `getEvent(name)`
- [x] Implement `getEntity(type)`
- [x] Implement `search(query, type, stability, limit, offset)` using `URLSearchParams`
- [x] Add minimal TS types (prefer `unknown` + narrowing over `any`)

### 8) Shared UI components (React parity)
- [x] Create `ui-react/src/components/StabilityBadge.tsx` (same mapping + labels as Svelte)
- [x] Create `ui-react/src/components/Markdown.tsx` using `marked` (block rendering)
- [x] Configure `marked` options like Svelte (`breaks`, `gfm`)
- [x] Create `ui-react/src/components/InlineMarkdown.tsx` using `marked.parse`
- [x] Strip `<p>` tags in InlineMarkdown exactly like Svelte (`/<\/??p>/g` equivalent)
- [x] Create `ui-react/src/components/Pagination.tsx` matching Svelte pagination window + ellipses

### 9) Page: Search (`/` and `/search`) — strict parity
- [ ] Create Search route component
- [ ] Implement state: query, type filter, stability filter, current page, loading, error
- [ ] Mirror `itemsPerPage = 50`
- [ ] Parse initial state from URL query params (`q`, `type`, `stability`, `page`)
- [ ] Trigger initial search even with empty query (browse mode)
- [ ] Update URL when search state changes (same param rules as Svelte)
- [ ] Search on typing and on Enter (same behavior)
- [ ] Render results list cards with DaisyUI classes matching Svelte
- [ ] Render result type badge + id + stability badge + deprecated styling
- [ ] Render `brief` using InlineMarkdown
- [ ] Implement top pagination UI parity
- [ ] Implement bottom pagination UI parity
- [ ] Implement empty state + error alert + loading spinner parity
- [ ] Ensure links to detail pages match Svelte route scheme

### 10) Page: Stats (`/stats`) — strict parity
- [x] Create Stats route component
- [x] Fetch `getRegistryStats()` on mount
- [x] Render loading spinner parity
- [x] Render error alert parity
- [x] Render stats cards (Attributes/Metrics/Spans/Events/Entities)
- [x] Ensure each stat card link matches Svelte search link query usage

### 11) Detail pages — strict parity

#### Attribute (`/attribute/:key`)
- [ ] Create Attribute detail route component
- [ ] Fetch attribute by key on mount/param change
- [ ] Implement loading + error states parity
- [ ] Implement copy-to-clipboard + temporary copied indicator (2s)
- [ ] Render deprecated warning block parity (note + renamed_to link)
- [ ] Render description + note using Markdown component
- [ ] Render type formatting parity (incl enum members)
- [ ] Render examples parity

#### Metric (`/metric/:name`)
- [ ] Create Metric detail route component
- [ ] Fetch metric by name
- [ ] Implement copy-to-clipboard parity
- [ ] Render instrument/unit/attributes summary cards parity
- [ ] Render attributes table parity (requirement level badge rules)

#### Span (`/span/:type`)
- [ ] Create Span detail route component
- [ ] Fetch span by type
- [ ] Implement copy-to-clipboard parity
- [ ] Render kind badge parity (default `internal`)
- [ ] Render attributes table parity (sampling relevant indicator)

#### Event (`/event/:name`)
- [ ] Create Event detail route component
- [ ] Fetch event by name
- [ ] Implement copy-to-clipboard parity
- [ ] Render attributes table parity

#### Entity (`/entity/:type`)
- [ ] Create Entity detail route component
- [ ] Fetch entity by type
- [ ] Implement copy-to-clipboard parity
- [ ] Render identity attributes table parity
- [ ] Render description attributes table parity

### 12) Page: Schema (`/schema`) — strict parity
- [ ] Create Schema route component
- [ ] Parse `schema` query param (default `ForgeRegistryV2`)
- [ ] Fetch schema JSON from `/api/v1/schema/:name`
- [ ] Render left definition list + root card parity
- [ ] Parse `type` query param (`root` or definition name)
- [ ] Implement “select definition” updates to URL (same semantics as Svelte)
- [ ] Port type formatting logic (array/map/union/allOf/oneOf/anyOf) for display parity
- [ ] Implement clickable type references navigation parity
- [ ] Verify browser back/forward behavior parity

### 13) Page: API Docs (`/api-docs`) — strict parity
- [x] Create API Docs route component
- [x] Load RapiDoc via CDN script (allowed)
- [x] Render `<rapi-doc>` with `spec-url="/api/v1/openapi.json"`
- [x] Implement theme sync via MutationObserver on `data-theme`
- [x] Port the exact light/dark color attributes mapping from Svelte
- [x] Ensure layout sizing/positioning matches Svelte (navbar + sidebar offsets)

### 14) Parity verification (manual, no tests)
- [ ] Build `ui-react` and run `preview`, smoke-test all routes
- [ ] Verify strict parity for: Search filters, pagination, link targets
- [ ] Verify strict parity for: detail pages tables + deprecated behavior
- [ ] Verify strict parity for: Schema page navigation and query params
- [ ] Verify strict parity for: theme persistence + RapiDoc theme update

### 15) Keep apps separated (side-by-side)
- [ ] Ensure `ui/` build continues unchanged
- [ ] Ensure `ui-react/` build output stays `ui-react/dist`
- [ ] Document how to run/build each app separately (`ui/` vs `ui-react/`)

### 16) Cutover (later, after parity is accepted)
- [ ] Update Rust embed from `ui/dist` to `ui-react/dist` (in `src/serve/ui.rs`)
- [ ] Update any server-side docs/comments referencing `ui/` build
- [ ] Confirm SPA fallback works with TanStack Router + chosen routing mode
- [ ] Delete `ui/` after cutover is complete and verified
