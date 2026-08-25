import { invoke } from "@tauri-apps/api/core";

export const SESSION_DIRECTORY_IDS = [
  "claude",
  "codex",
  "gemini",
  "grokbuild",
  "opencode",
  "openclaw",
  "hermes",
  "pi",
] as const;

export type SessionDirectoryId = (typeof SESSION_DIRECTORY_IDS)[number];

export interface Session2mdSettings {
  directoryOverrides: Partial<Record<SessionDirectoryId, string>>;
}

export interface Session2mdSettingsSnapshot extends Session2mdSettings {
  resolvedDirectories: Record<SessionDirectoryId, string>;
}

export const session2mdSettingsApi = {
  get: (): Promise<Session2mdSettingsSnapshot> =>
    invoke("get_session2md_settings"),

  save: (settings: Session2mdSettings): Promise<Session2mdSettingsSnapshot> =>
    invoke("save_session2md_settings", { settings }),
};
