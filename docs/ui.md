# UI Applications

Weaver provides a web UI application for browsing and interacting with semantic convention registries:

- **`ui/`**: React + Vite + TypeScript + TanStack Router

It provides:
- API endpoints (`/api/v1/*`)
- Tailwind CSS + DaisyUI styling
- Markdown rendering (via `marked`)
- RapiDoc for API documentation

## UI (`ui/`)

UI built with React.

### Prerequisites

```bash
cd ui
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

- Build output: `ui/dist/`
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

## Production Deployment

Currently, Rust server (`src/serve/ui.rs`) embeds the React UI build (`ui/dist`) for serving.


## API Server Requirements

The UI application requires the Weaver API server running:

```bash
# Start the Weaver server in a separate terminal
weaver serve
```

- Server runs on: `http://localhost:8080`
- The UI proxies `/api` requests to this server via Vite dev proxy
