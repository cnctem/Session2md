import { invoke } from "@tauri-apps/api/core";

export interface LocalImagePayload {
  /** Absolute path that was resolved on the Rust side. */
  path: string;
  mime: string;
  dataUrl: string;
}

export const localFilesApi = {
  /**
   * Open a local file, directory or image with the operating system.
   * Relative paths are resolved against `baseDir` (the session cwd).
   */
  async openPath(path: string, baseDir?: string | null): Promise<string> {
    return await invoke("open_local_path", {
      path,
      baseDir: baseDir ?? null,
    });
  },

  /**
   * Read a local image as a data URL so it can be inline-rendered in the
   * conversation without enabling a broad asset protocol.
   */
  async readImage(
    path: string,
    baseDir?: string | null,
  ): Promise<LocalImagePayload> {
    return await invoke("read_local_image", {
      path,
      baseDir: baseDir ?? null,
    });
  },
};
