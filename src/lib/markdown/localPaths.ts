export type MarkdownLinkTarget =
  | { kind: "external"; url: string }
  | { kind: "local"; path: string }
  | { kind: "unsupported"; url: string };

const WINDOWS_ABSOLUTE_PATH = /^[a-zA-Z]:[\\/]/;
const URI_SCHEME = /^[a-zA-Z][a-zA-Z0-9+.-]*:/;
const WWW_HOST = /^www\.[^\s/]+\.[^\s/]+/i;

interface AbsolutePathOptions {
  platform?: string;
}

/**
 * Classify a Markdown href/src so the renderer can decide whether it should be
 * opened externally, resolved as a local file, or ignored.
 *
 * - `http(s)://` URLs are external.
 * - `file://`, POSIX absolute paths, `~/` paths and Windows drive paths are local.
 * - Anything else without a URI scheme is a path relative to the session cwd.
 */
export const classifyMarkdownLink = (
  raw: string,
  options: AbsolutePathOptions = {},
): MarkdownLinkTarget => {
  const value = raw.trim();
  if (!value) {
    return { kind: "unsupported", url: raw };
  }

  // Anchors refer to the current document, not to the local filesystem.
  if (value.startsWith("#")) {
    return { kind: "unsupported", url: value };
  }

  // Protocol-relative URLs cannot be resolved without a base URL.
  if (value.startsWith("//")) {
    return { kind: "unsupported", url: value };
  }

  const lower = value.toLowerCase();
  if (lower.startsWith("http://") || lower.startsWith("https://")) {
    try {
      const parsed = new URL(value);
      if (parsed.protocol === "http:" || parsed.protocol === "https:") {
        return { kind: "external", url: parsed.href };
      }
    } catch {
      return { kind: "unsupported", url: value };
    }
  }

  if (WWW_HOST.test(value) && !URI_SCHEME.test(value)) {
    return { kind: "external", url: `https://${value}` };
  }

  if (lower.startsWith("file://")) {
    return { kind: "local", path: value };
  }

  const platform = options.platform ?? detectPlatform();
  const isAbsolutePosix =
    value.startsWith("/") || (platform !== "windows" && value.startsWith("\\"));
  const isAbsoluteWindows =
    WINDOWS_ABSOLUTE_PATH.test(value) || value.startsWith("\\\\");
  if (
    isAbsolutePosix ||
    isAbsoluteWindows ||
    value === "~" ||
    value.startsWith("~/") ||
    value.startsWith("~\\")
  ) {
    return { kind: "local", path: value };
  }

  if (URI_SCHEME.test(value)) {
    return { kind: "unsupported", url: value };
  }

  return { kind: "local", path: value };
};

const detectPlatform = (): string => {
  if (typeof navigator === "undefined") return "unknown";
  const userAgentData = (
    navigator as Navigator & { userAgentData?: { platform?: string } }
  ).userAgentData;
  const source = userAgentData?.platform ?? navigator.platform ?? "";
  return source.toLowerCase().includes("win") ? "windows" : "posix";
};

/**
 * Markdown renderers normalize link destinations as URIs, so non-ASCII path
 * segments arrive percent-encoded. Decode them once before handing the path to
 * the native side. `file://` URLs are decoded by the URL parser instead.
 */
export const decodeLocalPath = (value: string): string => {
  if (value.toLowerCase().startsWith("file://")) return value;
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
};

export const getPathBasename = (value: string): string => {
  const trimmed = value.trim().replace(/[\\/]+$/, "");
  if (!trimmed) return value;
  const segments = trimmed.split(/[\\/]/);
  return segments[segments.length - 1] || trimmed;
};
