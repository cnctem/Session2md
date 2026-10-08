import { useCallback, useState } from "react";
import {
  ArrowUpCircle,
  Download,
  ExternalLink,
  Loader2,
  X,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { MarkdownMessageContent } from "@/components/sessions/MarkdownMessageContent";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { AppUpdateInfo } from "@/lib/api/appUpdate";
import { sessionsApi } from "@/lib/api/sessions";
import { copyText } from "@/lib/clipboard";

interface AppUpdateDialogProps {
  update: AppUpdateInfo | null;
  currentVersion: string;
  onDismiss: () => void;
}

type OpeningAction = "download" | "release" | "notes";

export function AppUpdateDialog({
  update,
  currentVersion,
  onDismiss,
}: AppUpdateDialogProps) {
  const { t, i18n } = useTranslation();
  const [openingAction, setOpeningAction] = useState<OpeningAction | null>(
    null,
  );

  const openLink = useCallback(
    async (url: string, action: OpeningAction) => {
      setOpeningAction(action);
      try {
        await sessionsApi.openExternalUrl(url);
      } catch (error) {
        console.warn("[AppUpdateDialog] Failed to open link", error);
        toast.error(t("sessionSettings.updateDialog.openLinkFailed"));
      } finally {
        setOpeningAction(null);
      }
    },
    [t],
  );

  const handleCopyCode = useCallback(
    async (content: string) => {
      try {
        await copyText(content);
        toast.success(t("sessionManager.codeCopied"));
      } catch (error) {
        console.warn(
          "[AppUpdateDialog] Failed to copy release notes code",
          error,
        );
        toast.error(t("sessionSettings.updateDialog.copyFailed"));
      }
    },
    [t],
  );

  if (!update) {
    return null;
  }

  const version = update.latestVersion.startsWith("v")
    ? update.latestVersion
    : `v${update.latestVersion}`;
  const parsedPublishedAt = update.publishedAt
    ? new Date(update.publishedAt)
    : null;
  const publishedAt =
    parsedPublishedAt && !Number.isNaN(parsedPublishedAt.getTime())
      ? new Intl.DateTimeFormat(i18n.resolvedLanguage || i18n.language, {
          dateStyle: "medium",
        }).format(parsedPublishedAt)
      : null;
  const hasReleaseNotes = Boolean(update.releaseNotes?.trim());
  const isOpeningLink = openingAction !== null;

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onDismiss();
      }}
    >
      <DialogContent
        className="max-h-[90vh] max-w-2xl overflow-hidden"
        zIndex="top"
      >
        <DialogHeader className="relative pr-14">
          <DialogTitle className="flex items-center gap-2">
            <ArrowUpCircle className="size-5 text-blue-500" />
            {t("sessionSettings.updateDialog.title", { version })}
          </DialogTitle>
          <DialogDescription>
            {t("sessionSettings.updateDialog.description")}
          </DialogDescription>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="absolute right-5 top-4 size-7"
            aria-label={t("sessionSettings.updateDialog.close")}
            onClick={onDismiss}
          >
            <X className="size-4" />
          </Button>
        </DialogHeader>

        <div className="min-h-0 flex-1 space-y-4 overflow-y-auto px-6 py-5">
          <div className="grid gap-3 rounded-lg border bg-muted/25 p-4 sm:grid-cols-2">
            <div>
              <p className="text-xs text-muted-foreground">
                {t("sessionSettings.updateDialog.currentVersion")}
              </p>
              <p className="mt-1 font-mono text-sm font-medium">
                {currentVersion || t("common.unknown")}
              </p>
            </div>
            <div>
              <p className="text-xs text-muted-foreground">
                {t("sessionSettings.updateDialog.latestVersion")}
              </p>
              <p className="mt-1 font-mono text-sm font-medium text-blue-600 dark:text-blue-400">
                {version}
              </p>
            </div>
          </div>

          {(update.releaseName || publishedAt) && (
            <div className="space-y-1 text-sm">
              {update.releaseName && (
                <p className="font-medium">{update.releaseName}</p>
              )}
              {publishedAt && (
                <p className="text-xs text-muted-foreground">
                  {t("sessionSettings.about.updates.publishedAt", {
                    date: publishedAt,
                  })}
                </p>
              )}
            </div>
          )}

          {hasReleaseNotes && (
            <section className="space-y-2">
              <h3 className="text-sm font-semibold">
                {t("sessionSettings.updateDialog.releaseNotes")}
              </h3>
              <div className="max-h-[42vh] overflow-y-auto rounded-lg border bg-background/70 p-4 text-sm leading-6">
                <MarkdownMessageContent
                  content={update.releaseNotes ?? ""}
                  onCopyCode={(content) => void handleCopyCode(content)}
                  onOpenLink={(url) => void openLink(url, "notes")}
                />
              </div>
            </section>
          )}
        </div>

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            disabled={isOpeningLink}
            onClick={onDismiss}
          >
            {t("sessionSettings.updateDialog.later")}
          </Button>
          <Button
            type="button"
            variant="outline"
            disabled={isOpeningLink}
            onClick={() => void openLink(update.releaseUrl, "release")}
          >
            {openingAction === "release" ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <ExternalLink className="size-3.5" />
            )}
            {t("sessionSettings.about.updates.viewRelease")}
          </Button>
          <Button
            type="button"
            disabled={isOpeningLink}
            onClick={() => void openLink(update.downloadUrl, "download")}
          >
            {openingAction === "download" ? (
              <Loader2 className="size-3.5 animate-spin" />
            ) : (
              <Download className="size-3.5" />
            )}
            {t("sessionSettings.about.updates.download", { version })}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
