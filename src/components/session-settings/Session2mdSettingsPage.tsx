import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { FolderSearch, Loader2, Undo2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { LanguageSettings } from "@/components/settings/LanguageSettings";
import { ThemeSettings } from "@/components/settings/ThemeSettings";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { Button } from "@/components/ui/button";
import {
  SESSION_DIRECTORY_IDS,
  session2mdSettingsApi,
  type Session2mdSettings,
  type SessionDirectoryId,
} from "@/lib/api/session2mdSettings";
import {
  session2mdSettingsKey,
  useSession2mdSettingsQuery,
} from "@/lib/query/session2mdSettings";
import i18n from "@/i18n";

type LanguageOption = "zh" | "zh-TW" | "en" | "ja";

const LANGUAGE_STORAGE_KEY = "session2md-language";

const DIRECTORY_LABEL_KEYS: Record<SessionDirectoryId, string> = {
  claude: "sessionSettings.directories.claude",
  codex: "sessionSettings.directories.codex",
  gemini: "sessionSettings.directories.gemini",
  grokbuild: "sessionSettings.directories.grokbuild",
  opencode: "sessionSettings.directories.opencode",
  openclaw: "sessionSettings.directories.openclaw",
  hermes: "sessionSettings.directories.hermes",
  pi: "sessionSettings.directories.pi",
};

const asLanguageOption = (value: string): LanguageOption => {
  if (value === "zh" || value === "zh-TW" || value === "en" || value === "ja") {
    return value;
  }
  return "zh";
};

export function Session2mdSettingsPage() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { data: settings, isLoading, isError } = useSession2mdSettingsQuery();
  const [drafts, setDrafts] = useState<
    Partial<Record<SessionDirectoryId, string>>
  >({});
  const [language, setLanguage] = useState<LanguageOption>(() =>
    asLanguageOption(i18n.resolvedLanguage || i18n.language),
  );

  useEffect(() => {
    setDrafts({});
  }, [settings?.directoryOverrides]);

  const saveMutation = useMutation({
    mutationFn: (next: Session2mdSettings) => session2mdSettingsApi.save(next),
    onSuccess: async (snapshot) => {
      queryClient.setQueryData(session2mdSettingsKey, snapshot);
      await queryClient.invalidateQueries({ queryKey: ["sessions"] });
    },
  });

  const displayDirectories = useMemo(() => {
    if (!settings) return {} as Record<SessionDirectoryId, string>;

    return Object.fromEntries(
      SESSION_DIRECTORY_IDS.map((directoryId) => [
        directoryId,
        drafts[directoryId] ??
          settings.directoryOverrides[directoryId] ??
          settings.resolvedDirectories[directoryId],
      ]),
    ) as Record<SessionDirectoryId, string>;
  }, [drafts, settings]);

  const changeLanguage = (nextLanguage: LanguageOption) => {
    setLanguage(nextLanguage);
    window.localStorage.setItem(LANGUAGE_STORAGE_KEY, nextLanguage);
    void i18n.changeLanguage(nextLanguage);
  };

  const saveDirectory = async (
    directoryId: SessionDirectoryId,
    rawValue: string,
  ) => {
    if (!settings) return;

    const value = rawValue.trim();
    const nextOverrides = { ...settings.directoryOverrides };
    if (value) {
      nextOverrides[directoryId] = value;
    } else {
      delete nextOverrides[directoryId];
    }

    try {
      await saveMutation.mutateAsync({ directoryOverrides: nextOverrides });
      setDrafts((current) => {
        const next = { ...current };
        delete next[directoryId];
        return next;
      });
    } catch (error) {
      toast.error(
        t("sessionSettings.directorySaveFailed", {
          error: String(error),
        }),
      );
    }
  };

  const browseDirectory = async (directoryId: SessionDirectoryId) => {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: displayDirectories[directoryId],
    });
    if (typeof selected !== "string") return;

    setDrafts((current) => ({ ...current, [directoryId]: selected }));
    await saveDirectory(directoryId, selected);
  };

  const resetDirectory = async (directoryId: SessionDirectoryId) => {
    await saveDirectory(directoryId, "");
  };

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <Loader2 className="size-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (isError || !settings) {
    return (
      <div className="flex h-full items-center justify-center px-6 text-sm text-muted-foreground">
        {t("sessionSettings.loadFailed")}
      </div>
    );
  }

  return (
    <TooltipProvider>
      <div className="h-full overflow-y-auto">
        <div className="mx-auto w-full max-w-5xl px-5 py-6 sm:px-8">
          <Tabs defaultValue="general">
            <TabsList aria-label={t("sessionSettings.title")}>
              <TabsTrigger value="general">{t("settings.general")}</TabsTrigger>
              <TabsTrigger value="advanced">
                {t("settings.tabAdvanced")}
              </TabsTrigger>
            </TabsList>

            <TabsContent value="general" className="max-w-2xl space-y-8 py-6">
              <LanguageSettings value={language} onChange={changeLanguage} />
              <ThemeSettings />
            </TabsContent>

            <TabsContent value="advanced" className="max-w-3xl py-6">
              <section className="space-y-5">
                <header className="space-y-1">
                  <h2 className="text-base font-semibold">
                    {t("sessionSettings.directoryOverrides.title")}
                  </h2>
                  <p className="text-sm text-muted-foreground">
                    {t("sessionSettings.directoryOverrides.description")}
                  </p>
                </header>

                <div className="space-y-4">
                  {SESSION_DIRECTORY_IDS.map((directoryId) => (
                    <div key={directoryId} className="space-y-1.5">
                      <label
                        className="text-sm font-medium"
                        htmlFor={`session-directory-${directoryId}`}
                      >
                        {t(DIRECTORY_LABEL_KEYS[directoryId])}
                      </label>
                      <div className="flex items-center gap-2">
                        <Input
                          id={`session-directory-${directoryId}`}
                          value={displayDirectories[directoryId]}
                          onChange={(event) =>
                            setDrafts((current) => ({
                              ...current,
                              [directoryId]: event.target.value,
                            }))
                          }
                          onBlur={(event) =>
                            void saveDirectory(directoryId, event.target.value)
                          }
                          onKeyDown={(event) => {
                            if (event.key === "Enter")
                              event.currentTarget.blur();
                          }}
                        />
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              type="button"
                              variant="outline"
                              size="icon"
                              aria-label={t("sessionSettings.browseDirectory")}
                              onClick={() => void browseDirectory(directoryId)}
                            >
                              <FolderSearch className="size-4" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>
                            {t("sessionSettings.browseDirectory")}
                          </TooltipContent>
                        </Tooltip>
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              type="button"
                              variant="outline"
                              size="icon"
                              aria-label={t("sessionSettings.resetDirectory")}
                              onClick={() => void resetDirectory(directoryId)}
                            >
                              <Undo2 className="size-4" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>
                            {t("sessionSettings.resetDirectory")}
                          </TooltipContent>
                        </Tooltip>
                      </div>
                    </div>
                  ))}
                </div>
              </section>
            </TabsContent>
          </Tabs>
        </div>
      </div>
    </TooltipProvider>
  );
}
