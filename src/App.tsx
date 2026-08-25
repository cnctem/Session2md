import { ArrowLeft, FileDown, Settings } from "lucide-react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { SessionManagerPage } from "@/components/sessions/SessionManagerPage";
import { Session2mdSettingsPage } from "@/components/session-settings/Session2mdSettingsPage";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";

function App() {
  const { t } = useTranslation();
  const [page, setPage] = useState<"sessions" | "settings">("sessions");
  const isSettingsPage = page === "settings";

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
        {isSettingsPage ? <Session2mdSettingsPage /> : <SessionManagerPage />}
      </main>
    </div>
  );
}

export default App;
