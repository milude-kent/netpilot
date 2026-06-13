# NetPilot M2 — Web UI Design

Date: 2026-06-13

## Goal

Build a React-based NOC dashboard that provides real-time visibility into the NetPilot routing daemon. The Web UI communicates with the daemon's REST API and SSE event stream to display protocol status, route tables, configuration, and live events.

## Scope

### In scope

| Component | Description |
|-----------|-------------|
| Dashboard layout | Sidebar navigation + main content area, responsive |
| Protocol status panel | Cards per protocol showing state (Up/Down/Error), route count, uptime |
| Route table viewer | Searchable, sortable table of RIB entries with prefix/gateway/metric/protocol |
| Configuration editor | JSON editor pane for viewing and editing candidate configuration |
| SSE event log | Real-time streaming event log from `/api/events` |
| Health indicator | Green/yellow/red indicator based on `/health` endpoint |
| Embedded distribution | Build output served from daemon via `rust-embed` at `/` |

### Out of scope

- Authentication/login page
- Multi-daemon management (single daemon focus)
- Graph-based topology visualization
- gRPC/gNMI client in browser
- Mobile-optimized layout
- Dark mode toggle

## Architecture

```
netpilot-web/
  src/                    ← React application source
  src/App.tsx             ← Root component, router
  src/components/         ← Reusable UI components
  src/pages/              ← Page-level components
  src/hooks/              ← Custom React hooks
  src/api.ts              ← REST API client
  src/sse.ts              ← SSE EventSource handler
  dist/                   ← Production build output (embedded)
  package.json            ← Dependencies + build scripts
  vite.config.ts          ← Vite build configuration
```

## Component Tree

```
App
├── Sidebar
│   ├── NavLink("Dashboard")
│   ├── NavLink("Routes")
│   ├── NavLink("Protocols")
│   ├── NavLink("Events")
│   └── NavLink("Config")
├── MainContent
│   ├── DashboardPage
│   │   ├── HealthBadge
│   │   ├── ProtocolStatusCard[] (one per protocol)
│   │   └── QuickStats (route count, uptime, event rate)
│   ├── RoutesPage
│   │   ├── RouteSearchBar
│   │   └── RouteTable (prefix, gateway, metric, protocol, age)
│   ├── ProtocolsPage
│   │   └── ProtocolDetailCard[] (expandable, per-protocol stats)
│   ├── EventsPage
│   │   └── EventLog (scrollable, auto-scroll, filter by type)
│   └── ConfigPage
│       └── ConfigEditor (JSON textarea, diff highlight, commit button)
└── SSEProvider (context wrapping whole app)
```

## Key Data Flows

### SSE Event Stream
```
Browser ──EventSource──→ /api/events (SSE)
  → Event types:
    - state_change: ProtocolState { protocol, state, timestamp }
    - route_announce: RouteEntry { prefix, gateway, metric, protocol }
    - route_withdraw: RouteWithdraw { prefix, protocol }
    - error: ErrorEvent { protocol, message, timestamp }
    - stats: ProtocolStats { protocol, routes_count, uptime, event_rate }
  → SSEProvider parses and dispatches to React context
  → All subscribed components re-render on new events
```

### Configuration Workflow
```
ConfigPage:
  1. GET /api/config/candidate → populate JSON editor
  2. User edits in JSON editor
  3. PUT /api/config/candidate with edited JSON
  4. GET /api/config/diff → highlight changes
  5. POST /api/config/commit → apply
  (Rollback: POST /api/config/rollback)
```

### Polling Fallback
```
If SSE connection drops:
  - HealthBadge polls /health every 3s
  - ProtocolStatusCard polls /api/config/running every 5s (read-only)
  - RouteTable polls /api/routes every 5s
  - WebSocket/SSE reconnection with exponential backoff (1s, 2s, 4s, ... max 30s)
```

## Build & Embedding

The React app is built with Vite into `dist/`. At daemon build time:

```
1. cd crates/netpilot-web && npm run build
2. netpilotd build script (build.rs) detects dist/ and embeds it
3. Daemon serves dist/index.html at /, static assets at /assets/...
4. Single-page app routing: all unknown paths serve index.html
```

The embedded dist is served by `axum` with `tower-http` `ServeDir` pointing to the embedded assets. Content-Type headers are set from file extensions. Cache-Control is set to `no-cache` for development builds and `public, max-age=3600` for release.

## Design Decisions

1. **React + TypeScript + Vite**: Standard modern web stack. TypeScript for type safety across API contracts. Vite for fast builds and HMR during development.

2. **SSE over WebSocket**: SSE is simpler (unidirectional, HTTP-native), and the daemon only pushes events. Browser auto-reconnection is built into EventSource. No need for bidirectional WebSocket.

3. **Embedded, not separate service**: The web UI is served from the same daemon process. No CORS issues, no separate deployment, no API key management. Single binary distribution.

4. **Context-based state management**: React Context (SSEProvider) provides event state to the entire component tree. No Redux or external state library needed for this complexity level. Components subscribe via useContext.

5. **Polling fallback**: SSE can fail behind certain proxies. The UI degrades gracefully to polling when the EventSource connection is down.

6. **JSON config editor, not form-based**: The configuration schema is large and evolving. A JSON editor is simpler to maintain than a dynamic form builder. Syntax highlighting and validation are planned for later iterations.

For the canonical implementation, see `crates/netpilot-web/`.
