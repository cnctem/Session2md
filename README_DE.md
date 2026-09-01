# Session2md

Session2md ist eine eigenständige Tauri-Desktop-App zum Durchsuchen lokaler AI-CLI-Unterhaltungen und zum Export als Markdown.

Session2md basiert auf dem Open-Source-Projekt [CC Switch](https://github.com/farion1231/cc-switch). Das Projekt wird inzwischen unabhängig gepflegt und konzentriert sich auf das Durchsuchen, Verwalten und Exportieren lokaler Sitzungen.

[English](README.md) | [简体中文](README_ZH.md) | [日本語](README_JA.md) | Deutsch

## Funktionen

Session2md erkennt Sitzungen aus den folgenden lokal installierten Tools:

- Codex
- Claude Code
- Gemini CLI
- Grok Build
- OpenCode
- OpenClaw
- Hermes
- Pi

Für erkannte Sitzungen stehen folgende Funktionen zur Verfügung:

- Unterhaltung und Inhaltsverzeichnis anzeigen.
- Nach Sitzungs-ID, Titel, Zusammenfassung, Projektverzeichnis und Quellpfad suchen.
- Nach Anbieter filtern und zwischen flacher Liste sowie nach Anbieter/Projektverzeichnis gruppierter Ansicht wechseln.
- Projektverzeichnis, Quellpfad und verfügbare Wiederaufnahmebefehle des Anbieters kopieren.
- Die Unterhaltung über den nativen Speicherdialog als Markdown-Datei exportieren.
- Eine Sitzung löschen oder mehrere Sitzungen auswählen, auch ganze Anbieter- oder Verzeichnisgruppen.

## Daten und Sicherheit

Session2md liest die lokalen Sitzungsdaten der jeweiligen Tools direkt ein. Die App verbindet sich nicht mit CC Switch, startet keinen Proxy, lädt keine Gesprächsinhalte hoch und startet bzw. setzt keine CLI-Sitzung fort. Ein angezeigter Wiederaufnahmebefehl wird nur auf ausdrückliche Nutzeraktion in die Zwischenablage kopiert.

Das Löschen ist eine bestätigungspflichtige, destruktive Aktion. Nach der Bestätigung verwendet Session2md eine anbieterspezifische Bereinigung und ändert die ausgewählte lokale Sitzungsdatei oder Datenbank. Gelöschte Sitzungen können von der App nicht wiederhergestellt werden; wichtige Daten sollten vorher gesichert werden. Sprache, Design und Verzeichnisüberschreibungen der App werden separat gespeichert. Beim Export wird ausschließlich die im nativen Speicherdialog ausgewählte Markdown-Datei geschrieben.

## Sitzungsverzeichnisse

Standardmäßig verwendet Session2md die üblichen lokalen Verzeichnisse der Anbieter. Unter **Einstellungen → Erweitert** können die Verzeichnisse angezeigt oder pro Anbieter überschrieben werden; nach dem Speichern muss die Sitzungsliste aktualisiert werden. Das ist für portable Installationen oder mehrere Profile gedacht. Ohne Überschreibung berücksichtigt Codex außerdem `CODEX_HOME`, Hermes `HERMES_HOME` und OpenCode `XDG_DATA_HOME`.

## Download

Pakete für macOS, Linux und Windows sind auf der Seite [Releases](https://github.com/cnctem/Session2md/releases) verfügbar.

## Entwicklung

Benötigt werden Node.js, pnpm, Rust, Cargo und die [Tauri-Voraussetzungen](https://v2.tauri.app/start/prerequisites/).

```bash
pnpm install
pnpm dev
```

Vor dem Einreichen von Änderungen sollten die relevanten Prüfungen ausgeführt werden:

```bash
pnpm typecheck
pnpm format:check
pnpm test:unit
cargo fmt --check --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
```

Ein installierbares Paket wird mit folgendem Befehl erstellt:

```bash
pnpm tauri build
```

## Mitwirken

Fehlerberichte und fokussierte Pull Requests sind willkommen. Bitte lesen Sie [CONTRIBUTING.md](CONTRIBUTING.md) und [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Ursprung und Danksagung

Unser Dank gilt den Maintainerinnen und Maintainer von CC Switch sowie allen [CC-Switch-Mitwirkenden](https://github.com/farion1231/cc-switch/graphs/contributors). Session2md baut auf der offenen Tauri-Architektur, den Anbieterintegrationen und den Grundlagen der Sitzungsverwaltung von CC Switch auf. Beiträge zum [Session2md-Repository](https://github.com/cnctem/Session2md) sind ebenfalls willkommen.

## Lizenz

[MIT](LICENSE) © Jason Young
