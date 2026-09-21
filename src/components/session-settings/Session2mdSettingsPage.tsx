import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  Brain,
  FileText,
  FolderSearch,
  Loader2,
  TerminalSquare,
  Undo2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { LanguageSettings } from "@/components/settings/LanguageSettings";
import { ThemeSettings } from "@/components/settings/ThemeSettings";
import { SessionProviderIcon } from "@/components/sessions/SessionProviderIcon";
import { getProviderLabel } from "@/components/sessions/utils";
import { Input } from "@/components/ui/input";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Checkbox } from "@/components/ui/checkbox";
import { ToggleRow } from "@/components/ui/toggle-row";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { Button } from "@/components/ui/button";
import {
  session2mdSettingsApi,
  type Session2mdSettings,
} from "@/lib/api/session2mdSettings";
import {
  SESSION_DIRECTORY_IDS,
  SESSION_DIRECTORY_LABEL_KEYS,
  SESSION_PROVIDER_IDS,
  type SessionDirectoryId,
  type SessionProviderId,
} from "@/lib/sessionProviders";
import {
  session2mdSettingsKey,
  useSession2mdSettingsQuery,
} from "@/lib/query/session2mdSettings";
import i18n from "@/i18n";

type LanguageOption = "zh" | "zh-TW" | "en" | "ja";

const LANGUAGE_STORAGE_KEY = "session2md-language";

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
    onSuccess: (snapshot) => {
      queryClient.setQueryData(session2mdSettingsKey, snapshot);
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

  const hiddenProviders = useMemo(
    () => new Set(settings?.hiddenProviders ?? []),
    [settings?.hiddenProviders],
  );
  const allProvidersVisible = hiddenProviders.size === 0;
  const allProvidersHidden =
    hiddenProviders.size === SESSION_PROVIDER_IDS.length;

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
      await saveMutation.mutateAsync({
        directoryOverrides: nextOverrides,
        hiddenProviders: settings.hiddenProviders,
        exportThinking: settings.exportThinking,
        exportToolInputs: settings.exportToolInputs,
        exportToolOutputs: settings.exportToolOutputs,
      });
      await queryClient.invalidateQueries({ queryKey: ["sessions"] });
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

  const saveProviderVisibility = async (
    nextHiddenProviders: Set<SessionProviderId>,
  ) => {
    if (!settings) return;

    const previousSettings = settings;
    const hiddenProviderList = SESSION_PROVIDER_IDS.filter((providerId) =>
      nextHiddenProviders.has(providerId),
    );
    queryClient.setQueryData(session2mdSettingsKey, {
      ...settings,
      hiddenProviders: hiddenProviderList,
    });

    try {
      await saveMutation.mutateAsync({
        directoryOverrides: settings.directoryOverrides,
        hiddenProviders: hiddenProviderList,
        exportThinking: settings.exportThinking,
        exportToolInputs: settings.exportToolInputs,
        exportToolOutputs: settings.exportToolOutputs,
      });
      await queryClient.invalidateQueries({ queryKey: ["sessions"] });
    } catch (error) {
      queryClient.setQueryData(session2mdSettingsKey, previousSettings);
      toast.error(
        t("sessionSettings.providerVisibility.saveFailed", {
          error: String(error),
        }),
      );
    }
  };

  const setProviderVisible = async (
    providerId: SessionProviderId,
    visible: boolean,
  ) => {
    if (!settings) return;
    const nextHiddenProviders = new Set(settings.hiddenProviders);
    if (visible) {
      nextHiddenProviders.delete(providerId);
    } else {
      nextHiddenProviders.add(providerId);
    }
    await saveProviderVisibility(nextHiddenProviders);
  };

  const saveExportOption = async (
    key: "exportThinking" | "exportToolInputs" | "exportToolOutputs",
    value: boolean,
  ) => {
    if (!settings) return;

    const previousSettings = settings;
    const nextSettings = { ...settings, [key]: value };
    queryClient.setQueryData(session2mdSettingsKey, nextSettings);

    try {
      await saveMutation.mutateAsync({
        directoryOverrides: nextSettings.directoryOverrides,
        hiddenProviders: nextSettings.hiddenProviders,
        exportThinking: nextSettings.exportThinking,
        exportToolInputs: nextSettings.exportToolInputs,
        exportToolOutputs: nextSettings.exportToolOutputs,
      });
    } catch (error) {
      queryClient.setQueryData(session2mdSettingsKey, previousSettings);
      toast.error(
        t("sessionSettings.exportContent.saveFailed", {
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
              <section className="space-y-5">
                <header className="space-y-1">
                  <h2 className="text-base font-semibold">
                    {t("sessionSettings.exportContent.title")}
                  </h2>
                  <p className="text-sm text-muted-foreground">
                    {t("sessionSettings.exportContent.description")}
                  </p>
                </header>

                <div className="space-y-3">
                  <ToggleRow
                    icon={<Brain className="size-4 text-blue-500" />}
                    title={t(
                      "sessionSettings.exportContent.includeThinking.label",
                    )}
                    description={t(
                      "sessionSettings.exportContent.includeThinking.description",
                    )}
                    checked={settings.exportThinking}
                    onCheckedChange={(value) =>
                      void saveExportOption("exportThinking", value)
                    }
                    disabled={saveMutation.isPending}
                  />
                  <ToggleRow
                    icon={<TerminalSquare className="size-4 text-green-500" />}
                    title={t(
                      "sessionSettings.exportContent.includeToolInputs.label",
                    )}
                    description={t(
                      "sessionSettings.exportContent.includeToolInputs.description",
                    )}
                    checked={settings.exportToolInputs}
                    onCheckedChange={(value) =>
                      void saveExportOption("exportToolInputs", value)
                    }
                    disabled={saveMutation.isPending}
                  />
                  <ToggleRow
                    icon={<FileText className="size-4 text-cyan-500" />}
                    title={t(
                      "sessionSettings.exportContent.includeToolOutputs.label",
                    )}
                    description={t(
                      "sessionSettings.exportContent.includeToolOutputs.description",
                    )}
                    checked={settings.exportToolOutputs}
                    onCheckedChange={(value) =>
                      void saveExportOption("exportToolOutputs", value)
                    }
                    disabled={saveMutation.isPending}
                  />
                </div>
              </section>

              <section className="space-y-5">
                <header className="space-y-1">
                  <h2 className="text-base font-semibold">
                    {t("sessionSettings.providerVisibility.title")}
                  </h2>
                  <p className="text-sm text-muted-foreground">
                    {t("sessionSettings.providerVisibility.description")}
                  </p>
                </header>

                <div className="flex flex-wrap items-center gap-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={saveMutation.isPending || allProvidersVisible}
                    onClick={() => void saveProviderVisibility(new Set())}
                  >
                    {t("sessionSettings.providerVisibility.selectAll")}
                  </Button>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={saveMutation.isPending || allProvidersHidden}
                    onClick={() =>
                      void saveProviderVisibility(new Set(SESSION_PROVIDER_IDS))
                    }
                  >
                    {t("sessionSettings.providerVisibility.clearAll")}
                  </Button>
                </div>

                <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
                  {SESSION_PROVIDER_IDS.map((providerId) => {
                    const checkboxId = `session-provider-visible-${providerId}`;
                    return (
                      <label
                        key={providerId}
                        htmlFor={checkboxId}
                        className="flex cursor-pointer items-center gap-2 rounded-md border px-3 py-2 text-sm transition-colors hover:bg-muted/50"
                      >
                        <Checkbox
                          id={checkboxId}
                          checked={!hiddenProviders.has(providerId)}
                          disabled={saveMutation.isPending}
                          onCheckedChange={(checked) =>
                            void setProviderVisible(
                              providerId,
                              checked === true,
                            )
                          }
                        />
                        <span aria-hidden="true">
                          <SessionProviderIcon
                            providerId={providerId}
                            size={16}
                          />
                        </span>
                        <span className="min-w-0 truncate">
                          {getProviderLabel(providerId, t)}
                        </span>
                      </label>
                    );
                  })}
                </div>
              </section>
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
                        {t(SESSION_DIRECTORY_LABEL_KEYS[directoryId])}
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
