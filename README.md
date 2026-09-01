# Session2md

Session2md is a standalone Tauri desktop app for browsing local AI CLI conversations and exporting them as Markdown.

Session2md is based on the open-source [CC Switch](https://github.com/farion1231/cc-switch) project. It is maintained as a separate project, with a focused scope for browsing, managing, and exporting local sessions.

[简体中文](README_ZH.md) | [日本語](README_JA.md) | [Deutsch](README_DE.md)

## What it does

Session2md discovers sessions from these local tools:

- Codex
- Claude Code
- Gemini CLI
- Grok Build
- OpenCode
- OpenClaw
- Hermes
- Pi

For each discovered session you can:

- Browse the conversation and its table of contents.
- Search session IDs, titles, summaries, project directories, and source paths.
- Filter by provider and switch between a flat list or provider/project-directory groups.
- Copy the project directory, source path, or an available provider resume command.
- Export the loaded conversation to a Markdown file through the native save dialog.
- Delete one session or select multiple sessions, including whole provider or directory groups.

## Data and safety

Session2md reads the providers' local session stores. It does not connect to CC Switch, run a proxy, upload session content, or start/resume a CLI session. A displayed resume command is only copied to the clipboard when requested.

Deletion is explicit and destructive. After confirmation, Session2md performs provider-specific cleanup in the selected local session file or database; deleted sessions cannot be recovered by the app. Back up important data before using deletion. The app also stores its own language, theme, and directory-override settings separately. Markdown export writes only the file chosen in the native save dialog.

## Source directories

By default, Session2md follows each provider's normal local directory. Open **Settings → Advanced** to inspect or override the directory used for any provider, then refresh the session list. Overrides are useful for portable setups or alternate profiles. Codex also respects `CODEX_HOME`, Hermes respects `HERMES_HOME`, and OpenCode respects `XDG_DATA_HOME` when no override is configured.

## Download

Packaged builds for macOS, Linux, and Windows are published on the [Releases](https://github.com/cnctem/Session2md/releases) page.

## Development

Prerequisites: Node.js, pnpm, Rust, Cargo, and the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
pnpm install
pnpm dev
```

Run the relevant checks before submitting changes:

```bash
pnpm typecheck
pnpm format:check
pnpm test:unit
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

Build an installable bundle with:

```bash
pnpm tauri build
```

## Contributing

Bug reports and focused pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Origin and acknowledgments

Thank you to the CC Switch maintainers and all [CC Switch contributors](https://github.com/farion1231/cc-switch/graphs/contributors) for the original open-source Tauri architecture, provider integrations, and session-browser foundations on which Session2md builds. We also welcome contributions to the [Session2md repository](https://github.com/cnctem/Session2md).

## License

[MIT](LICENSE) © Jason Young
