import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { localFilesApi } from "@/lib/api/localFiles";
import {
  describeLocalFileError,
  parseLocalFileError,
  showLocalFileErrorToast,
} from "@/lib/markdown/localFileErrors";
import {
  classifyMarkdownLink,
  decodeLocalPath,
  getPathBasename,
} from "@/lib/markdown/localPaths";

interface LoadedImage {
  src: string;
  resolvedPath?: string;
}

type ImageState =
  | { status: "loading" }
  | { status: "ready"; image: LoadedImage }
  | { status: "error"; error?: unknown };

const imageCache = new Map<string, Promise<LoadedImage>>();
const MAX_CACHED_IMAGES = 32;

export const clearMarkdownImageCache = () => {
  imageCache.clear();
};

const cacheKey = (path: string, baseDir?: string | null) =>
  `${baseDir ?? ""}\u0000${path}`;

const loadLocalImage = (
  path: string,
  baseDir?: string | null,
): Promise<LoadedImage> => {
  const key = cacheKey(path, baseDir);
  const cached = imageCache.get(key);
  if (cached) return cached;

  const request = localFilesApi
    .readImage(path, baseDir)
    .then((payload) => ({
      src: payload.dataUrl,
      resolvedPath: payload.path,
    }))
    .catch((error: unknown) => {
      imageCache.delete(key);
      throw error;
    });

  if (imageCache.size >= MAX_CACHED_IMAGES) {
    const oldest = imageCache.keys().next().value;
    if (oldest) imageCache.delete(oldest);
  }
  imageCache.set(key, request);
  return request;
};

interface MarkdownImageProps {
  src?: string;
  alt?: string;
  baseDir?: string | null;
  onOpenLink: (url: string) => void;
}

export function MarkdownImage({
  src,
  alt,
  baseDir,
  onOpenLink,
}: MarkdownImageProps) {
  const { t } = useTranslation();
  const [state, setState] = useState<ImageState>(
    src ? { status: "loading" } : { status: "error" },
  );

  useEffect(() => {
    if (!src) {
      setState({ status: "error" });
      return;
    }

    const classified = classifyMarkdownLink(src);
    if (classified.kind === "external") {
      setState({ status: "ready", image: { src: classified.url } });
      return;
    }
    if (classified.kind !== "local") {
      setState({ status: "error" });
      return;
    }

    let cancelled = false;
    setState({ status: "loading" });
    loadLocalImage(decodeLocalPath(classified.path), baseDir)
      .then((image) => {
        if (!cancelled) setState({ status: "ready", image });
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setState({ status: "error", error });
        }
      });

    return () => {
      cancelled = true;
    };
  }, [src, baseDir]);

  const label =
    alt?.trim() || (src ? getPathBasename(decodeLocalPath(src)) : "");

  if (state.status === "error") {
    if (!label) return null;

    const localizedError = state.error
      ? describeLocalFileError(state.error, t)
      : null;
    const title = localizedError
      ? [localizedError.title, localizedError.description]
          .filter(Boolean)
          .join("\n")
      : undefined;
    const isMissing = parseLocalFileError(state.error).code === "fileNotFound";

    return (
      <span
        className="my-2 inline-block rounded border border-border/70 bg-muted/50 px-2 py-1 text-xs text-muted-foreground"
        title={title}
      >
        {isMissing
          ? t("sessionManager.imageNotFound", {
              alt: label,
              defaultValue: "[Image not found: {{alt}}]",
            })
          : t("sessionManager.imageLoadFailed", {
              alt: label,
              defaultValue: "[Image failed to load: {{alt}}]",
            })}
      </span>
    );
  }

  if (state.status === "loading") {
    return (
      <span className="my-2 inline-flex items-center gap-2 rounded border border-border/70 bg-muted/40 px-2 py-1 text-xs text-muted-foreground">
        {t("sessionManager.loadingImage", {
          defaultValue: "Loading image…",
        })}
      </span>
    );
  }

  const openImage = () => {
    if (state.image.resolvedPath) {
      void localFilesApi
        .openPath(state.image.resolvedPath, baseDir)
        .catch((error: unknown) => showLocalFileErrorToast(error, t));
      return;
    }
    onOpenLink(state.image.src);
  };

  return (
    <span className="my-2 block max-w-full">
      <button
        type="button"
        onClick={openImage}
        aria-label={
          alt?.trim() ||
          t("sessionManager.openImage", { defaultValue: "Open image" })
        }
        title={
          state.image.resolvedPath ??
          t("sessionManager.openImage", { defaultValue: "Open image" })
        }
        className="block max-w-full cursor-zoom-in overflow-hidden rounded-lg border border-border/70 bg-muted/20"
      >
        <img
          src={state.image.src}
          alt={label}
          loading="lazy"
          referrerPolicy="no-referrer"
          className="block h-auto max-h-[460px] w-auto max-w-full object-contain"
          onError={() => setState({ status: "error" })}
        />
      </button>
      {alt?.trim() ? (
        <span className="mt-1 block text-xs text-muted-foreground">{alt}</span>
      ) : null}
    </span>
  );
}
