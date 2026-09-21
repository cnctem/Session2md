# Session2md

Session2md は、ローカルの AI CLI 会話を閲覧し、Markdown として書き出せる独立した Tauri デスクトップアプリです。

Session2md はオープンソースの [CC Switch](https://github.com/farion1231/cc-switch) を基に開発されています。現在は独立して保守され、ローカルセッションの閲覧、管理、書き出しに集中しています。

[English](README.md) | [简体中文](README_ZH.md) | 日本語 | [Deutsch](README_DE.md)

## 機能

次のローカルツールのセッションを検出できます。

- Codex
- Claude Code
- Gemini CLI
- Grok Build
- OpenCode
- OpenClaw
- Hermes
- Pi
- Cursor
- Antigravity CLI
- Reasonix
- MiMo Code
- ZCode
- Kimi CLI / Kimi Code
- Kilo Code
- Qoder CLI
- WorkBuddy
- Qwen Work
- Continue
- Cline
- Goose
- Zed
- Crush
- TeleAgent
- DeepSeek Harness

検出したセッションでは、次の操作ができます。

- 会話本文と目次を閲覧する。
- セッション ID、タイトル、概要、プロジェクトディレクトリ、ソースパスを検索する。
- プロバイダーで絞り込み、フラット表示またはプロバイダー/プロジェクトディレクトリ別のグループ表示に切り替える。
- プロジェクトディレクトリ、ソースパス、利用可能なプロバイダーの再開コマンドをコピーする。
- OS の保存ダイアログから会話を Markdown ファイルに書き出す。
- 既存の削除対応プロバイダーのセッションを削除する。または複数選択し、プロジェクトディレクトリ単位で削除する。新規追加の読み取り専用プロバイダーには削除 UI を表示しません。

## データと安全性

Session2md は各ツールのローカルセッションストアを直接読み取ります。CC Switch には接続せず、プロキシを起動せず、会話内容をアップロードせず、CLI セッションを起動・再開しません。画面に表示される再開コマンドは、ユーザーが操作した場合にコピーされるだけです。

削除はアプリが削除対応とマークしたプロバイダーだけで利用でき、確認が必要な破壊的操作です。確認後、Session2md はプロバイダー固有の処理で選択したローカルのセッションファイルまたはデータベースを変更します。削除したセッションはアプリから復元できないため、重要なデータは事前にバックアップしてください。アプリの言語、テーマ、ディレクトリ上書き設定は別途保存されます。エクスポートで書き込まれるのは、OS の保存ダイアログで選択した Markdown ファイルだけです。

## セッションディレクトリ

初期状態では、各プロバイダーの標準的なローカルディレクトリを使用します。**設定 → 詳細** でプロバイダーごとのディレクトリを確認・上書きでき、保存後にセッション一覧を更新すると反映されます。ポータブル構成や複数プロファイルに便利です。上書きがない場合、Codex は `CODEX_HOME`、Hermes は `HERMES_HOME`、OpenCode は `XDG_DATA_HOME` も利用します。

## ダウンロード

macOS、Linux、Windows 用のパッケージは [Releases](https://github.com/cnctem/Session2md/releases) で公開しています。

## 開発

Node.js、pnpm、Rust、Cargo、および [Tauri の前提環境](https://v2.tauri.app/start/prerequisites/) が必要です。

```bash
pnpm install
pnpm dev
```

変更を提出する前に、関連するチェックを実行してください。

```bash
pnpm typecheck
pnpm format:check
pnpm test:unit
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

インストール用パッケージをビルドするには、次を実行します。

```bash
pnpm tauri build
```

## コントリビューション

バグ報告と、目的を絞った Pull Request を歓迎します。[CONTRIBUTING.md](CONTRIBUTING.md) と [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) を確認してください。

## プロジェクトの由来と謝辞

CC Switch のメンテナーとすべての [CC Switch コントリビューター](https://github.com/farion1231/cc-switch/graphs/contributors) に感謝します。Session2md は、CC Switch のオープンソース Tauri アーキテクチャ、プロバイダー連携、セッションブラウザーの基盤を引き継いでいます。[Session2md リポジトリ](https://github.com/cnctem/Session2md) への改善提案や貢献も歓迎します。

## ライセンス

[MIT](LICENSE) © Jason Young
