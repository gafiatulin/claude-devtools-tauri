# Profiling Guide

Three complementary tools for understanding where time is spent: the Vite bundle visualizer for frontend payload, Rust `tracing` spans for backend command latency, and React/Preact DevTools for render cost.

---

## 1. Bundle Report (Vite)

`vite-plugin-visualizer` generates `dist/stats.html` on every production build.

```sh
npm run build
open dist/stats.html
```

The treemap shows every module's gzip size. Use it to:

- Confirm the three manual chunks (`settings`, `dashboard`, `markdown`) are **not** included in `index.js`
- Find unexpectedly large transitive dependencies
- Verify that `react-markdown` / `unified` appear only inside the `markdown` chunk

**Expected post-split sizes (gzip):**

| Chunk | Size |
|---|---|
| `index.js` (main) | ~119 kB |
| `markdown` | ~47 kB |
| `dashboard` | ~42 kB |
| `settings` | ~15 kB |

If a chunk appears in the main bundle, check for a synchronous top-level import of that module in an eagerly-loaded file.

---

## 2. Rust Command Timing (tracing)

All session and search Tauri commands are instrumented with `#[tracing::instrument]`. Spans are emitted to stderr when running the dev build.

**Enable timing output:**

```sh
RUST_LOG=claude_devtools_tauri=debug cargo tauri dev
```

Or to see only our spans at `info` level (the default):

```sh
cargo tauri dev
```

The subscriber initialised in `lib.rs::run()` uses `EnvFilter` defaulting to `info` for `claude_devtools_tauri`. Each instrumented function emits an entry + exit span:

```
INFO claude_devtools_tauri::commands::sessions: close time.busy=2.31ms time.idle=14µs
INFO claude_devtools_tauri::commands::sessions: scan_sessions time.busy=18.4ms time.idle=9µs
```

**Instrumented commands** (in `commands/sessions.rs` and `commands/search.rs`):

- `get_session_detail`
- `scan_sessions` / `scan_sessions_paginated`
- `search_sessions`
- and all other session commands

**Adjusting verbosity:**

```sh
# All spans including internal library noise
RUST_LOG=trace cargo tauri dev

# Only our crate at debug
RUST_LOG=claude_devtools_tauri=debug cargo tauri dev

# Suppress tracing entirely
RUST_LOG=error cargo tauri dev
```

---

## 3. React / Preact DevTools Profiling

The app ships with React 18 (or Preact compat after the Preact migration) using `createRoot`, which enables concurrent mode and the React DevTools profiler.

### Setup

1. Install the [React Developer Tools](https://chrome.google.com/webstore/detail/react-developer-tools/fmkadmapgofadopljbjfkapdkoienihi) browser extension.
2. In Tauri dev mode, open the webview inspector: right-click → "Inspect Element".
3. Switch to the **Profiler** tab.

### Recording a profile

1. Click **Record** (circle icon).
2. Perform the interaction you want to measure — e.g., open a large session, scroll, switch tabs.
3. Click **Stop**.

### Reading the flamegraph

- **Gray bars** = components that did not re-render.
- **Colored bars** = components that re-rendered; color intensity indicates relative cost.
- Look for wide bars in `ChatHistory`, `DateGroupedSessions`, and `SessionContextPanel` — these are the performance-critical paths.

### What to look for

| Symptom | Likely cause |
|---|---|
| `SessionContextPanel` re-renders on every keystroke | `useDeferredValue` not applied, or deferred value not memoized |
| Entire session list re-renders on tab switch | `useShallow` missing from store selector |
| `MarkdownViewer` renders synchronously on first load | `LazyMarkdownViewer` wrapper not in place |
| `ChatHistory` re-renders without conversation change | Unstable prop reference — wrap parent in `React.memo` or stabilise with `useCallback` |

### Preact DevTools

After the Preact migration (`react → preact/compat` alias), the React DevTools extension continues to work because Preact compat exposes the same `__REACT_DEVTOOLS_GLOBAL_HOOK__` interface. No additional setup required.

---

## Baseline Numbers (recorded 2026-02-17)

| Metric | Value |
|---|---|
| Main bundle (gzip) | 119.26 kB |
| Full bundle (all chunks, gzip) | ~219 kB |
| Production DMG size | 4.3 MB |
| `scan_sessions` (100 sessions) | ~18 ms |
| `get_session_detail` (typical) | ~2 ms |
| Rust test suite | 128 tests, 0 failures |
