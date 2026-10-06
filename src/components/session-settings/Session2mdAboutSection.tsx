import { useState } from "react";
import {
  AlertCircle,
  CheckCircle2,
  Download,
  ExternalLink,
  Github,
  Loader2,
  RefreshCw,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import appIcon from "@/assets/icons/app-icon.png";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ToggleRow } from "@/components/ui/toggle-row";
import type { AppUpdateController } from "@/hooks/useAppUpdate";
import { sessionsApi } from "@/lib/api/sessions";

interface Session2mdAboutSectionProps {
  update: AppUpdateController;
  autoCheckUpdates: boolean;
  isSavingAutoCheckUpdates: boolean;
  onAutoCheckUpdatesChange: (value: boolean) => void;
}

const REPOSITORY_URL = "https://github.com/cnctem/Session2md";

export function Session2mdAboutSection({
  update,
  autoCheckUpdates,
  isSavingAutoCheckUpdates,
  onAutoCheckUpdatesChange,
}: Session2mdAboutSectionProps) {
  const { t, i18n } = useTranslation();
  const [isOpeningLink, setIsOpeningLink] = useState(false);
  const { currentVersion, updateInfo, isChecking, hasChecked, error, check } =
    update;

  const openLink = async (url: string) => {
    setIsOpeningLink(true);
    try {
      await sessionsApi.openExternalUrl(url);
    } catch (openError) {
      console.warn("[Session2mdAboutSection] Failed to open link", openError);
      toast.error(t("sessionSettings.about.openLinkFailed"));
    } finally {
      setIsOpeningLink(false);
    }
  };

  const handleCheckUpdate = async () => {
    try {
      const result = await check();
      if (!result) {
        toast.success(t("sessionSettings.about.upToDate"));
      }
    } catch {
      toast.error(t("sessionSettings.about.checkFailed"));
    }
  };

  const publishedAt = updateInfo?.publishedAt
    ? new Intl.DateTimeFormat(i18n.resolvedLanguage || i18n.language, {
        dateStyle: "medium",
      }).format(new Date(updateInfo.publishedAt))
    : null;

  const status = (() => {
    if (isChecking) {
      return {
        icon: <Loader2 className="size-4 animate-spin" />,
        text: t("sessionSettings.about.checking"),
        tone: "text-muted-foreground",
      };
    }
    if (error) {
      return {
        icon: <AlertCircle className="size-4 text-destructive" />,
        text: t("sessionSettings.about.checkFailed"),
        tone: "text-destructive",
      };
    }
    if (updateInfo) {
      return {
        icon: <Download className="size-4 text-blue-500" />,
        text: t("sessionSettings.about.updateAvailable", {
          version: updateInfo.latestVersion,
        }),
        tone: "text-blue-600 dark:text-blue-400",
      };
    }
    if (hasChecked) {
      return {
        icon: <CheckCircle2 className="size-4 text-emerald-500" />,
        text: t("sessionSettings.about.upToDate"),
        tone: "text-emerald-600 dark:text-emerald-400",
      };
    }
    return {
      icon: <RefreshCw className="size-4" />,
      text: t("sessionSettings.about.notChecked"),
      tone: "text-muted-foreground",
    };
  })();

  return (
    <section className="max-w-3xl space-y-5 py-6">
      <header className="space-y-1">
        <h2 className="text-base font-semibold">
          {t("sessionSettings.about.title")}
        </h2>
        <p className="text-sm text-muted-foreground">
          {t("sessionSettings.about.description")}
        </p>
      </header>

      <div className="space-y-5 rounded-xl border border-border bg-gradient-to-br from-card/80 to-card/40 p-6 shadow-sm">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
          <div className="flex min-w-0 items-start gap-4">
            <img
              src={appIcon}
              alt="Session2md"
              className="size-12 shrink-0 rounded-xl shadow-sm"
            />
            <div className="min-w-0 space-y-2">
              <div className="flex flex-wrap items-center gap-2">
                <h3 className="text-lg font-semibold">Session2md</h3>
                <Badge variant="outline" className="bg-background/80">
                  {t("common.version")} {currentVersion || t("common.loading")}
                </Badge>
              </div>
              <p className="text-sm leading-6 text-muted-foreground">
                {t("sessionSettings.about.appDescription")}
              </p>
            </div>
          </div>

          <div className="flex shrink-0 flex-wrap gap-2">
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={isOpeningLink}
              onClick={() => void openLink(REPOSITORY_URL)}
            >
              <Github className="size-3.5" />
              {t("sessionSettings.about.github")}
            </Button>
            <Button
              type="button"
              variant="outline"
              size="sm"
              disabled={isOpeningLink}
              onClick={() =>
                void openLink(
                  updateInfo?.releaseUrl ?? `${REPOSITORY_URL}/releases/latest`,
                )
              }
            >
              <ExternalLink className="size-3.5" />
              {t("sessionSettings.about.releases")}
            </Button>
          </div>
        </div>

        <p className="border-t pt-4 text-sm leading-6 text-muted-foreground">
          {t("sessionSettings.about.localFirstDescription")}
        </p>
      </div>

      <div className="space-y-4 rounded-xl border border-border bg-card/50 p-6 shadow-sm">
        <header className="space-y-1">
          <h3 className="text-sm font-semibold">
            {t("sessionSettings.about.updates.title")}
          </h3>
          <p className="text-xs leading-5 text-muted-foreground">
            {t("sessionSettings.about.updates.description")}
          </p>
        </header>

        <ToggleRow
          icon={<RefreshCw className="size-4 text-blue-500" />}
          title={t("sessionSettings.about.updates.autoCheck.label")}
          description={t("sessionSettings.about.updates.autoCheck.description")}
          checked={autoCheckUpdates}
          onCheckedChange={onAutoCheckUpdatesChange}
          disabled={isSavingAutoCheckUpdates}
        />

        <div
          aria-live="polite"
          className="flex flex-col gap-3 rounded-lg border bg-background/70 p-4 sm:flex-row sm:items-center sm:justify-between"
        >
          <div className="flex min-w-0 items-start gap-3">
            <span className={`mt-0.5 shrink-0 ${status.tone}`}>
              {status.icon}
            </span>
            <div className="min-w-0">
              <p className={`text-sm font-medium ${status.tone}`}>
                {status.text}
              </p>
              {updateInfo?.releaseName && (
                <p className="mt-1 truncate text-xs text-muted-foreground">
                  {updateInfo.releaseName}
                </p>
              )}
              {publishedAt && (
                <p className="mt-1 text-xs text-muted-foreground">
                  {t("sessionSettings.about.updates.publishedAt", {
                    date: publishedAt,
                  })}
                </p>
              )}
            </div>
          </div>

          <div className="flex shrink-0 flex-wrap gap-2">
            {updateInfo ? (
              <>
                <Button
                  type="button"
                  size="sm"
                  disabled={isOpeningLink}
                  onClick={() => void openLink(updateInfo.downloadUrl)}
                >
                  <Download className="size-3.5" />
                  {t("sessionSettings.about.updates.download", {
                    version: updateInfo.latestVersion,
                  })}
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  disabled={isOpeningLink}
                  onClick={() => void openLink(updateInfo.releaseUrl)}
                >
                  <ExternalLink className="size-3.5" />
                  {t("sessionSettings.about.updates.viewRelease")}
                </Button>
              </>
            ) : (
              <Button
                type="button"
                variant="outline"
                size="sm"
                disabled={isChecking}
                onClick={() => void handleCheckUpdate()}
              >
                <RefreshCw
                  className={isChecking ? "size-3.5 animate-spin" : "size-3.5"}
                />
                {hasChecked
                  ? t("sessionSettings.about.updates.recheck")
                  : t("sessionSettings.about.updates.checkNow")}
              </Button>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}
