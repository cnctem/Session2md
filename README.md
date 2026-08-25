# Session2md

Session2md is a standalone desktop application for browsing local AI CLI conversations and exporting them as Markdown.

## Supported Sources

- Codex
- Claude Code
- Gemini CLI
- Grok Build
- OpenCode
- OpenClaw
- Hermes
- Pi

Session2md reads these local session stores directly. It does not read or write CC Switch configuration, does not start a proxy, and does not modify or resume source sessions. The only write is the Markdown file selected in the native save dialog.

## Development

```bash
pnpm install
pnpm dev
```

```bash
pnpm typecheck
pnpm test:unit
cd src-tauri && cargo check
```
