# Session2md

Session2md 是一款独立的 Tauri 桌面应用，用于浏览本地 AI CLI 会话，并将对话导出为 Markdown。

Session2md 基于开源项目 [CC Switch](https://github.com/farion1231/cc-switch) 开发。目前作为独立项目维护，专注于本地会话的浏览、管理和导出。

[English](README.md) | 简体中文 | [日本語](README_JA.md) | [Deutsch](README_DE.md)

## 功能

Session2md 可以从以下本地工具发现会话：

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
- 千问办公
- Continue
- Cline
- Goose
- Zed
- Crush
- TeleAgent
- DeepSeek Harness

对发现的会话可以进行以下操作：

- 浏览完整对话和目录大纲。
- 搜索会话 ID、标题、摘要、项目目录和源文件路径。
- 按供应商筛选，并在平铺列表或“供应商/项目目录”分组视图之间切换。
- 复制项目目录、源路径，以及部分供应商提供的恢复命令。
- 通过系统保存对话框将当前对话导出为 Markdown 文件。
- 删除支持写入能力的既有供应商会话，或多选并按项目目录整组删除。新增来源默认只读，不提供删除入口。

## 数据与安全

Session2md 直接读取各工具的本地会话存储，不连接 CC Switch，不启动代理，不上传会话内容，也不会启动或恢复 CLI 会话。界面中的恢复命令仅在用户主动操作时复制到剪贴板。

只有应用标记为可删除的供应商才提供删除操作，并且该操作需要确认且不可逆。确认后，Session2md 会通过对应供应商的清理逻辑修改本地会话文件或数据库；应用无法恢复已删除的会话，请在删除前备份重要数据。应用自身的语言、主题和目录覆盖设置会单独保存。导出功能只会写入用户在系统保存对话框中选择的 Markdown 文件。

## 会话目录

默认情况下，Session2md 使用各供应商的标准本地目录。进入 **设置 → 高级** 可以查看或覆盖任意供应商的目录，保存后刷新会话列表即可生效，适合便携配置或多套配置目录。未设置覆盖目录时，Codex 还会读取 `CODEX_HOME`，Hermes 读取 `HERMES_HOME`，OpenCode 读取 `XDG_DATA_HOME`。

## 下载

macOS、Linux 和 Windows 的安装包发布在 [Releases](https://github.com/cnctem/Session2md/releases) 页面。

## 开发

准备 Node.js、pnpm、Rust、Cargo，以及 [Tauri 所需环境](https://v2.tauri.app/start/prerequisites/)。

```bash
pnpm install
pnpm dev
```

提交修改前运行相关检查：

```bash
pnpm typecheck
pnpm format:check
pnpm test:unit
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

构建可安装的软件包：

```bash
pnpm tauri build
```

## 贡献

欢迎提交问题反馈和聚焦明确的 Pull Request。请先阅读 [CONTRIBUTING.md](CONTRIBUTING.md) 和 [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)。

## 项目来源与开源致谢

感谢 CC Switch 的维护者以及所有 [CC Switch 贡献者](https://github.com/farion1231/cc-switch/graphs/contributors)！Session2md 继承并延续了 CC Switch 的开源 Tauri 架构、供应商适配和会话浏览基础。也欢迎通过 [Session2md 仓库](https://github.com/cnctem/Session2md) 提交改进和贡献。

## 许可证

[MIT](LICENSE) © Jason Young
