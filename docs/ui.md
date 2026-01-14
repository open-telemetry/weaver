# UI Applications

Weaver provides two separate web UI applications for browsing and interacting with semantic convention registries:

- **`ui/`**: Svelte + Vite + TypeScript (original UI)
- **`ui-react/`**: React + Vite + TypeScript + TanStack Router (new UI, in development)

Both applications are fully functional and provide feature parity. They share the same:
- API endpoints (`/api/v1/*`)
- Tailwind CSS + DaisyUI styling
- Markdown rendering (via `marked`)
- RapiDoc for API documentation

## Svelte UI (`ui/`)

The original UI built with Svelte.

### Prerequisites

```bash
cd ui
npm install
```

### Development

```bash
npm run dev
```

- Dev server runs on: `http://localhost:4173`
- API proxy: `/api` → `http://localhost:8080`

### Build

```bash
npm run build
```

- Build output: `ui/dist/`
- Ready for production deployment

### Preview

```bash
npm run preview
```

Preview the production build locally.

### Tech Stack

- **Framework**: Svelte 5
- **Build tool**: Vite 7
- **Routing**: svelte-spa-router (hash-based)
- **Styling**: Tailwind CSS v3 + DaisyUI 4
- **Markdown**: marked
- **API docs**: RapiDoc (via CDN)

## React UI (`ui-react/`)

New UI built with React, currently in active development.

### Prerequisites

```bash
cd ui-react
npm install
```

### Development

```bash
npm run dev
```

- Dev server runs on: `http://localhost:5173`
- API proxy: `/api` → `http://localhost:8080`

### Build

```bash
npm run build
```

- Build output: `ui-react/dist/`
- Ready for production deployment

### Preview

```bash
npm run preview
```

Preview the production build locally.

### Tech Stack

- **Framework**: React 19
- **Build tool**: Vite 7
- **Routing**: TanStack Router (file-based)
- **Styling**: Tailwind CSS v4 + DaisyUI 5.5
- **Markdown**: marked
- **API docs**: RapiDoc (via CDN)
- **State**: React hooks (no external state management)

## Running Both Apps Simultaneously

Both apps can run simultaneously in development mode on different ports:

```bash
# Terminal 1 - Svelte UI
cd ui && npm run dev

# Terminal 2 - React UI
cd ui-react && npm run dev
```

Then navigate to:
- Svelte UI: `http://localhost:4173`
- React UI: `http://localhost:5173`

## Production Deployment

Currently, the Rust server (`src/serve/ui.rs`) embeds the Svelte UI build (`ui/dist`) for serving. The React UI build (`ui-react/dist`) is not yet embedded.

To switch the embedded UI to React, update the Rust embed path in `src/serve/ui.rs` from `ui/dist` to `ui-react/dist` after verifying feature parity.

## API Server Requirements

Both UI applications require the Weaver API server running:

```bash
# Start the Weaver server in a separate terminal
weaver serve
```

- Server runs on: `http://localhost:8080`
- Both UIs proxy `/api` requests to this server via Vite dev proxy
