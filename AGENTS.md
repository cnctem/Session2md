# Session2md Contributor Guide

Session2md is a standalone Tauri desktop app for browsing local Agent CLI
conversation histories and exporting them as Markdown. In addition to Codex,
Claude Code, Gemini CLI, Grok Build, OpenCode, OpenClaw, Hermes, and Pi, it
supports Cursor, Antigravity CLI, Reasonix, MiMo Code, ZCode, Kimi CLI/Code,
Kilo Code, Qoder CLI, WorkBuddy, Qwen Work, Continue, Cline, Goose, Zed, Crush,
TeleAgent, and DeepSeek Harness.

## Product Boundary

- Source session stores are read-only. Do not modify, resume, migrate, delete,
  or otherwise write to an agent's session files or configuration.
- The only user-data write in this app is an explicitly selected Markdown export
  through the native save dialog.
- This repository was separated from CC Switch and still contains inherited
  modules. Keep Session2md work within the session browser and its settings
  unless the task explicitly requires a wider CC Switch feature.

## Architecture

- `src/components/sessions/` contains the session browser, message rendering,
  search, TOC, and Markdown formatting UI.
- `src/lib/api/sessions.ts` wraps Tauri `invoke` calls, and
  `src/lib/query/sessions.ts` owns the matching TanStack Query hooks.
- `src-tauri/src/commands/session_manager.rs` is the Tauri command boundary.
- `src-tauri/src/session_manager/` discovers local session stores and normalizes
  every provider into `SessionMeta` and `SessionMessage`.
- `src-tauri/src/session2md_settings.rs` and the session settings page configure
  optional source-directory overrides.

Keep the Rust-to-TypeScript contract in camelCase. When adding a provider,
implement scanning and message loading in a dedicated provider module, register
it with the session manager, connect it through the command and API layers, and
add it to the UI filter, labels, icons, settings, and locales as applicable.

## Implementation Rules

- Use `pnpm` (pinned in `package.json`); do not replace it with npm or yarn.
- TypeScript is strict. Use the `@/` alias for `src/` imports and keep query,
  API, component, and type responsibilities separate.
- Use existing shadcn/Radix components and Lucide icons for UI changes. Add
  translated strings to every supported locale rather than hard-coding UI text.
- Session parsers must tolerate missing, partial, and malformed local data. Skip
  unusable records rather than failing the full scan, and avoid logging session
  contents or credentials.
- Prefer focused parser fixtures and unit tests for provider format changes.
  Tests must use temporary paths or configured overrides, never real home
  directories, local agent stores, or user credentials.
- Do not edit ignored build output such as `dist/`, `src-tauri/target/`, or
  `src-tauri/gen/schemas/`.
- Do not commit, change remotes, or publish releases unless explicitly asked.

## Development And Verification

```bash
pnpm install
pnpm dev
```

Run the checks relevant to changed code before handoff:

```bash
pnpm format:check
pnpm typecheck
pnpm test:unit
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

For Rust session-parser changes, also run the focused library tests, then the
full Rust suite when the change reaches shared command or storage behavior:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib session_manager
cargo test --manifest-path src-tauri/Cargo.toml
```

Run `pnpm tauri dev` for UI flows that depend on native dialogs or local session
discovery. Preserve unrelated worktree changes and report any skipped or failed
checks in the handoff.
