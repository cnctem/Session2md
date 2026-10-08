import { invoke } from "@tauri-apps/api/core";

export interface AppUpdateInfo {
  latestVersion: string;
  releaseName: string | null;
  releaseNotes: string | null;
  releaseUrl: string;
  downloadUrl: string;
  publishedAt: string | null;
}

export const appUpdateApi = {
  async check(): Promise<AppUpdateInfo | null> {
    return await invoke("check_app_update");
  },
};
