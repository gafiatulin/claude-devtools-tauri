> **Personal adaptation — read before using.**
>
> This is a **personal, idiosyncratic fork** of [claude-devtools](https://github.com/matt1398/claude-devtools) by [@matt1398](https://github.com/matt1398). All credit for the original concept, feature set, and design goes to the original project. If you want a maintained, cross-platform tool, use that one.
>
> This fork is a **vibecoding experiment** — built entirely through AI-assisted sessions using Claude Code. It is not officially supported, not regularly maintained, and may break without warning or fix. The changes made here (Tauri rewrite, macOS-only, SSH removed, performance overhaul) reflect personal preferences, not a general product direction.
>
> **Target audience:** If you use Claude Code or similar AI tools, you have everything you need to adapt this to your own preferences — the same way this fork was built. Fork it, change what bothers you, ignore what doesn't apply. That's the point.

---

<p align="center">
  <img src="src/renderer/favicon.png" alt="claude-devtools" width="120" />
</p>

<h1 align="center">claude-devtools</h1>

<p align="center">
  <strong><code>Terminal tells you nothing. This shows you everything.</code></strong>
  <br />
  A native macOS app that reconstructs exactly what Claude Code did — every file path, every tool call, every token — from the raw session logs already on your machine.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/platform-macOS%20(Apple%20Silicon%20%2B%20Intel)-lightgrey?style=flat-square" alt="Platform" />
</p>

<p align="center">
  <sub>No API keys. No configuration. Just build, open, and see everything Claude Code did.</sub>
</p>

---

## Key Features

### Visible Context Reconstruction

Claude Code doesn't expose what's actually in the context window. claude-devtools reverse-engineers it.

The engine walks each turn of the session and reconstructs the full set of context injections — **CLAUDE.md files** (broken down by global, project, and directory-level), **skill activations**, **@-mentioned files**, **tool call inputs and outputs**, **extended thinking**, **team coordination overhead**, and **user prompt text**.

The result is a per-turn breakdown of estimated token attribution across 7 categories, surfaced in three places: a **Context Badge** on each assistant response, a **Token Usage popover** with percentage breakdowns, and a dedicated **Session Context Panel**.

### Compaction Visualization

**See the moment your context hits the limit.**

When Claude Code hits its context limit, it silently compresses your conversation and continues. claude-devtools detects these compaction boundaries, measures the token delta before and after, and visualizes how your context fills, compresses, and refills over the course of a session.

### Custom Notification Triggers

Define rules for when you want to receive **system notifications**. Match on regex patterns, assign colors, and filter your inbox by trigger.

- **Built-in defaults**: `.env File Access Alert`, `Tool Result Error` (`is_error: true`), and `High Token Usage` (default: 8,000 total tokens)
- **Custom matching**: use regex against specific fields like `file_path`, `command`, `prompt`, `content`, `thinking`, or `text`
- **Sensitive-file monitoring**: create alerts for `.env`, `secrets`, payment/billing paths, or any project-specific pattern

### Rich Tool Call Inspector

Every tool call is paired with its result in an expandable card. Specialized viewers render each tool natively:
- **Read** calls show syntax-highlighted code with line numbers
- **Edit** calls show inline diffs with added/removed highlighting
- **Bash** calls show command output
- **Subagent** calls show the full execution tree, expandable in-place

### Team & Subagent Visualization

Claude Code spawns subagents via the Task tool and coordinates entire teams via `TeamCreate`, `SendMessage`, and `TaskUpdate`. claude-devtools untangles it.

- **Subagent sessions** are resolved from Task tool calls and rendered as expandable inline cards — each with its own tool trace, token metrics, duration, and cost
- **Teammate messages** are detected and rendered as distinct color-coded cards, separated from regular user messages
- **Team lifecycle** is fully visible: `TeamCreate` initialization, task coordination, `SendMessage` direct messages and broadcasts, shutdown requests/responses, and `TeamDelete` teardown

### Command Palette & Cross-Session Search

Hit **Cmd+K** for a Spotlight-style command palette. Search across all sessions in a project — results show context snippets with highlighted keywords.

### Multi-Pane Layout

Open multiple sessions side-by-side. Drag-and-drop tabs between panes, split views, and compare sessions in parallel.

---

## What the CLI Hides vs. What claude-devtools Shows

| What you see in the terminal | What claude-devtools shows you |
|------------------------------|-------------------------------|
| `Read 3 files` | Exact file paths, syntax-highlighted content with line numbers |
| `Searched for 1 pattern` | The regex pattern, every matching file, and the matched lines |
| `Edited 2 files` | Inline diffs with added/removed highlighting per file |
| A three-segment context bar | Per-turn token attribution across 7 categories — CLAUDE.md breakdown, skills, @-mentions, tool I/O, thinking, teams, user text — with compaction visualization |
| Subagent output interleaved with the main thread | Isolated execution trees per agent, expandable inline with their own metrics |
| Teammate messages buried in session logs | Color-coded teammate cards with full team lifecycle visibility |
| Critical events mixed into normal output | Trigger-filtered notification inbox for `.env` access, execution errors, and high token usage |
| `--verbose` JSON dump | Structured, filterable, navigable interface — no noise |

---

## Differences from the Original

This rewrite trades cross-platform support and some advanced features for a dramatically smaller, faster native app. Several features were removed by personal choice (macOS-only machine, no SSH workflow, no Docker use case) — not because Tauri can't support them.

### What was removed (by choice)

| Feature | Original | This fork |
|---------|----------|-------|
| **Platform support** | macOS, Windows, Linux, Docker | macOS only |
| **SSH remote sessions** | Connect to remote machines over SSH/SFTP | Removed — not needed |
| **Auto-updater** | OTA updates via GitHub releases | Removed — manual updates |
| **Docker / standalone server** | Full Node.js HTTP server, deployable anywhere | Removed — local-only use |
| **Context switching** | Multiple workspaces with snapshot/restore | Removed — single machine |

### What improved

| Metric | Original | This fork |
|--------|----------|-------|
| App size | ~100 MB DMG | **6.1 MB DMG** (~94% reduction) |
| Backend | Node.js (interpreted) | Rust (compiled, native) |
| Session parsing | TypeScript JSONL parser | Rust parser with parallel scanning (Rayon) |
| Session cache | In-memory (process heap) | LRU cache with `Arc<Session>` (capacity 500, mtime-validated) |
| Metrics cache | None | Separate LRU cache; no re-parse on repeated metric reads |
| Heavy computation | Main thread | Two dedicated Web Workers (context tracking + AI group enhancement) |
| Bundle size (gzip) | - | ~219 KB total (code-split) |

---

## Installation

### Build locally (recommended)

Requires Rust (stable), pnpm, and [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
pnpm install
pnpm tauri build --bundles app
open src-tauri/target/release/bundle/macos/Claude\ DevTools.app
```

### Download from GitHub Releases

The app is not notarized with Apple, so macOS will block it on first open. After opening the DMG and dragging the app to Applications:

1. Open the app — macOS will show a warning and block it
2. Go to **System Settings → Privacy & Security**
3. Scroll down to find **"Claude DevTools" was blocked** and click **Open Anyway**

Alternatively, remove the quarantine attribute before opening:

```bash
xattr -d com.apple.quarantine /Applications/Claude\ DevTools.app
```

---

## Development

```bash
# Install dependencies
pnpm install

# Start dev server
pnpm tauri dev

# Build optimised app bundle (.app)
pnpm tauri build --bundles app
```

### Scripts

| Command | Description |
|---------|-------------|
| `pnpm tauri dev` | Dev server with hot reload |
| `pnpm build` | Frontend production build only |
| `pnpm tsc` | TypeScript type checking |
| `cargo test` | Rust unit tests (130 tests) |

---

## Architecture

**Rust backend (`src-tauri/`):**
- Reads `~/.claude/projects/**/*.jsonl` directly via native file I/O
- Parses JSONL into typed domain models
- Exposes 46 Tauri commands (`invoke()`)
- File watching via `notify` crate (FSEvents on macOS); channel-based debouncer (300ms, zero CPU when idle)
- LRU session cache with `Arc<Session>` (capacity 500, mtime-validated — no explicit eviction needed)
- Separate LRU metrics cache; `get_session_metrics` skips re-parsing on cache hit
- 2-second TTL projects cache shared across `get_projects` and `get_repository_groups`
- Parallel project/session/mention scanning via Rayon (8-thread pool)
- Full-text search engine; session files stat'd once before sort (not inside comparator)
- Notification trigger evaluator with pre-compiled regex cache
- Session metrics derived from chunk data — one fewer full pass over messages per detail load
- Lazy `JsonlIterator` — reads one JSONL entry per `next()` call with a reused line buffer; `read_jsonl_file()` is now a `collect()` wrapper
- Enhanced chunk `raw_messages` eliminated — replaced with a zero-cost `enhanced: true` marker; no more per-chunk deep clones of all messages, wire payload no longer contains duplicate message arrays

**TypeScript frontend (`src/renderer/`):**
- React 18 with Preact compat alias (runtime swap, same API)
- Zustand store with 12 slices; ChatHistory uses individual `===`-compared selectors (not `useShallow` objects)
- Code-split: vendor / main / dashboard / markdown / settings chunks
- Lazy markdown rendering via `React.lazy` + Suspense
- Two Web Workers: context tracking (`contextWorker`) and AI group enhancement (`aiGroupEnhancer.worker`, 10.7 KB chunk)
- `reviveDates()` uses in-place tree traversal with length pre-check (not JSON.parse/stringify roundtrip)
- In-flight session fetch deduplication — concurrent requests for same session share one promise
- Virtual list threshold: 80 items (was 120); height constants shared via `virtualListConstants.ts`

**Shared types (`src/shared/`):**
- `ElectronAPI` interface — the full Tauri command surface, implemented by `TauriClient` in `src/renderer/api/tauriClient.ts`

```
src-tauri/src/
├── commands/      # 46 Tauri commands
├── config/        # ConfigManager (RwLock, atomic writes)
├── jsonl/         # JSONL reader + parser
├── models/        # Domain types (serde)
├── notifications/ # Trigger evaluator + store
├── parser/        # Chunk builder, semantic extractor, tool linker
├── scanner/       # Project/session discovery
├── search/        # Full-text search engine
└── watcher/       # FSEvents file watcher
```

See [`docs/profiling.md`](docs/profiling.md) for bundle analysis, Rust tracing, and React DevTools instructions.

## License

[MIT](LICENSE)
