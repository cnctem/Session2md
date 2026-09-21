import { invoke } from "@tauri-apps/api/core";
import {
  SESSION_DIRECTORY_IDS,
  type SessionDirectoryId,
  type SessionProviderId,
} from "@/lib/sessionProviders";

export { SESSION_DIRECTORY_IDS };
export type { SessionDirectoryId };

export interface Session2mdSettings {
  directoryOverrides: Partial<Record<SessionDirectoryId, string>>;
  hiddenProviders: SessionProviderId[];
  exportThinking: boolean;
  exportToolInputs: boolean;
  exportToolOutputs: boolean;
  defaultExpandThinking: boolean;
  defaultExpandTools: boolean;
  defaultExpandSystem: boolean;
  renderMarkdown: boolean;
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
