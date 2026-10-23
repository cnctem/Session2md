import { toast } from "sonner";

import { extractErrorMessage } from "@/utils/errorUtils";

type Translate = (key: string, options?: Record<string, unknown>) => string;

export interface LocalFileErrorInfo {
  code?: string;
  path?: string;
  message: string;
}

export interface LocalFileErrorDescription {
  title: string;
  description?: string;
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null;

/**
 * Commands such as `open_local_path` reject with a structured error:
 * `{ code: "fileNotFound", message, path }`. Older errors are plain strings,
 * so keep both shapes working.
 */
export const parseLocalFileError = (error: unknown): LocalFileErrorInfo => {
  if (isRecord(error)) {
    const code = typeof error.code === "string" ? error.code : undefined;
    const path = typeof error.path === "string" ? error.path : undefined;
    const message = typeof error.message === "string" ? error.message : "";
    if (code || path || message) {
      return {
        code,
        path,
        message: message || extractErrorMessage(error),
      };
    }
  }

  return { message: extractErrorMessage(error) };
};

/**
 * Turn a local-file command failure into localized copy. Missing files get a
 * dedicated message with the resolved path plus a hint, since that is the most
 * common failure when reviewing old conversations.
 */
export const describeLocalFileError = (
  error: unknown,
  t: Translate,
): LocalFileErrorDescription => {
  const info = parseLocalFileError(error);

  if (info.code === "fileNotFound") {
    return {
      title: t("sessionManager.fileNotFound", {
        path: info.path || info.message,
        defaultValue: "File not found: {{path}}",
      }),
      description: t("sessionManager.fileNotFoundHint", {
        defaultValue:
          "The file may have been moved or deleted, or the link points to the wrong session directory.",
      }),
    };
  }

  return {
    title: t("sessionManager.openPathFailed", {
      error:
        info.message || t("common.unknown", { defaultValue: "Unknown error" }),
      defaultValue: "Failed to open file: {{error}}",
    }),
  };
};

export const showLocalFileErrorToast = (error: unknown, t: Translate) => {
  const { title, description } = describeLocalFileError(error, t);
  toast.error(title, description ? { description } : undefined);
};
