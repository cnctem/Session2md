import { ArrowLeft, Feather, FileDown, Settings } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { SessionManagerPage } from "@/components/sessions/SessionManagerPage";
import { AppUpdateDialog } from "@/components/session-settings/AppUpdateDialog";
import { AutoUpdatePreferenceDialog } from "@/components/session-settings/AutoUpdatePreferenceDialog";
import { Session2mdSettingsPage } from "@/components/session-settings/Session2mdSettingsPage";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { useAppUpdate } from "@/hooks/useAppUpdate";
import { useSession2mdSettingsQuery } from "@/lib/query/session2mdSettings";
import { cn } from "@/lib/utils";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

const MINIMAL_MODE_STORAGE_KEY = "session2md.minimalMode";

const readInitialMinimalMode = () => {
  if (typeof window === "undefined") return false;
  return window.localStorage.getItem(MINIMAL_MODE_STORAGE_KEY) === "true";
};

function App() {
  const { t } = useTranslation();
  const [page, setPage] = useState<"sessions" | "settings">("sessions");
  const [minimalMode, setMinimalMode] = useState(readInitialMinimalMode);
  const isSettingsPage = page === "settings";
  const { data: settings } = useSession2mdSettingsQuery();
  const appUpdate = useAppUpdate(settings?.autoCheckUpdates);
  const pendingAutoUpdatePreference =
    settings?.autoCheckUpdates === null ? settings : null;

  useEffect(() => {
    window.localStorage.setItem(MINIMAL_MODE_STORAGE_KEY, String(minimalMode));
  }, [minimalMode]);

  const handleMinimalModeChange = (checked: boolean) => {
    setMinimalMode(checked);
    if (checked) {
      toast.success(t("common.minimalModeEnabled"), {
        description: t("common.minimalModeDescription"),
      });
    } else {
      toast.info(t("common.minimalModeDisabled"));
    }
  };

  return (
    <div className="flex h-screen min-h-0 flex-col overflow-hidden bg-background text-foreground">
      <header className="flex h-14 shrink-0 items-center gap-3 border-b px-5">
        <div className="flex size-7 items-center justify-center rounded-md bg-primary text-primary-foreground">
          <FileDown className="size-4" />
        </div>
        <h1 className="text-sm font-semibold">Session2md</h1>
        <span className="text-muted-foreground">/</span>
        <span className="text-sm text-muted-foreground">
          {isSettingsPage
            ? t("sessionSettings.title")
            : t("sessionManager.title")}
        </span>
        {!isSettingsPage && (
          <div
            className="ml-2 flex h-8 shrink-0 items-center gap-1 rounded-lg bg-muted/50 px-1.5 transition-all"
            title={`${t("common.minimalMode")}\n${t("common.minimalModeDescription")}`}
          >
            <Feather
              className={cn(
                "h-4 w-4 transition-colors",
                minimalMode
                  ? "text-emerald-500 status-heartbeat"
                  : "text-muted-foreground",
              )}
            />
            <Switch
              checked={minimalMode}
              onCheckedChange={handleMinimalModeChange}
              aria-label={t("common.minimalMode")}
            />
          </div>
        )}
        <div className="ml-auto">
          <TooltipProvider>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  aria-label={
                    isSettingsPage
                      ? t("sessionSettings.backToSessions")
                      : t("common.settings")
                  }
                  onClick={() =>
                    setPage(isSettingsPage ? "sessions" : "settings")
                  }
                >
                  {isSettingsPage ? (
                    <ArrowLeft className="size-4" />
                  ) : (
                    <Settings className="size-4" />
                  )}
                </Button>
              </TooltipTrigger>
              <TooltipContent>
                {isSettingsPage
                  ? t("sessionSettings.backToSessions")
                  : t("common.settings")}
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
      </header>
      <main className="min-h-0 flex-1">
        {isSettingsPage ? (
          <Session2mdSettingsPage update={appUpdate} />
        ) : (
          <SessionManagerPage minimalMode={minimalMode} />
        )}
      </main>
      <AutoUpdatePreferenceDialog settings={pendingAutoUpdatePreference} />
      <AppUpdateDialog
        update={appUpdate.startupUpdateInfo}
        currentVersion={appUpdate.currentVersion}
        onDismiss={appUpdate.dismissStartupUpdate}
      />
    </div>
  );
}

export default App;
