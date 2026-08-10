import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Download,
  FolderOpen,
  List,
  ListTree,
  RefreshCw,
  Search,
  Clock,
} from "lucide-react";
import { toast } from "sonner";
import { useSessionSearch } from "@/hooks/useSessionSearch";
import { useSessionMessagesQuery, useSessionsQuery } from "@/lib/query/sessions";
import { sessionsApi } from "@/lib/api/sessions";
import { extractErrorMessage } from "@/utils/errorUtils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { SessionMeta } from "@/types";
import { SessionItem } from "./SessionItem";
import { SessionMessageItem } from "./SessionMessageItem";
import { SessionProviderIcon } from "./SessionProviderIcon";
import { SessionTocDialog, SessionTocSidebar } from "./SessionToc";
import {
  extractCodexPromptPreview,
  formatSessionMarkdown,
  formatSessionMessagePreview,
  formatSessionTitle,
  formatTimestamp,
  getProviderLabel,
  getSessionMarkdownFileName,
  getSessionKey,
  groupSessionsByProviderAndDirectory,
  shouldHideCodexMessageFromToc,
} from "./utils";

type ProviderFilter =
  | "all"
  | "codex"
  | "grokbuild"
  | "claude"
  | "opencode"
  | "openclaw"
  | "gemini"
  | "hermes"
  | "pi";

type SessionListViewMode = "flat" | "grouped";

const PROVIDER_FILTERS: ProviderFilter[] = [
  "all",
  "codex",
  "grokbuild",
  "claude",
  "opencode",
  "openclaw",
  "gemini",
  "hermes",
  "pi",
];

const LIST_VIEW_MODE_STORAGE_KEY = "session2md.sessionManager.listViewMode";

const readInitialListViewMode = (): SessionListViewMode => {
  if (typeof window === "undefined") return "flat";
  const stored = window.localStorage.getItem(LIST_VIEW_MODE_STORAGE_KEY);
  return stored === "grouped" ? "grouped" : "flat";
};

export function SessionManagerPage() {
  const { t } = useTranslation();
  const { data, isLoading, isFetching, refetch } = useSessionsQuery();
  const sessions = data ?? [];
  const [providerFilter, setProviderFilter] = useState<ProviderFilter>("all");
  const [search, setSearch] = useState("");
  const [selectedKey, setSelectedKey] = useState<string | null>(null);
  const [listViewMode, setListViewMode] = useState<SessionListViewMode>(
    readInitialListViewMode,
  );
  const [isExporting, setIsExporting] = useState(false);
  const [tocDialogOpen, setTocDialogOpen] = useState(false);
  const [activeMessageIndex, setActiveMessageIndex] = useState<number | null>(
    null,
  );
  const messageRefs = useRef(new Map<number, HTMLDivElement>());

  const { search: searchSessions } = useSessionSearch({
    sessions,
    providerFilter,
  });
  const filteredSessions = useMemo(
    () => searchSessions(search),
    [search, searchSessions],
  );
  const groupedSessions = useMemo(
    () =>
      groupSessionsByProviderAndDirectory(
        filteredSessions,
        t("sessionManager.unknownDirectory", { defaultValue: "未知目录" }),
      ),
    [filteredSessions, t],
  );

  useEffect(() => {
    window.localStorage.setItem(LIST_VIEW_MODE_STORAGE_KEY, listViewMode);
  }, [listViewMode]);

  useEffect(() => {
    if (filteredSessions.length === 0) {
      setSelectedKey(null);
      return;
    }
    if (!filteredSessions.some((session) => getSessionKey(session) === selectedKey)) {
      setSelectedKey(getSessionKey(filteredSessions[0]));
    }
  }, [filteredSessions, selectedKey]);

  const selectedSession = useMemo(
    () =>
      filteredSessions.find((session) => getSessionKey(session) === selectedKey) ?? null,
    [filteredSessions, selectedKey],
  );
  const { data: messages = [], isLoading: isLoadingMessages } =
    useSessionMessagesQuery(selectedSession?.providerId, selectedSession?.sourcePath);
  const isCodexSession = selectedSession?.providerId === "codex";
  const hasExportableMessages = messages.some((message) => {
    const role = message.role.toLowerCase();
    return (role === "user" || role === "assistant") && message.content.trim().length > 0;
  });
  const tocItems = useMemo(
    () =>
      messages
        .map((message, index) => ({ message, index }))
        .filter(({ message }) => {
          return (
            message.role.toLowerCase() === "user" &&
            !(isCodexSession && shouldHideCodexMessageFromToc(message.content))
          );
        })
        .map(({ message, index }) => ({
          index,
          preview: formatSessionMessagePreview(
            isCodexSession ? extractCodexPromptPreview(message.content) : message.content,
          ),
          ts: message.ts,
        })),
    [isCodexSession, messages],
  );

  const copyText = async (value: string, successMessage: string) => {
    try {
      await navigator.clipboard.writeText(value);
      toast.success(successMessage);
    } catch (error) {
      toast.error(extractErrorMessage(error) || t("common.error"));
    }
  };

  const handleExportMarkdown = async () => {
    if (!selectedSession || isExporting) return;
    const exportMessages = isCodexSession
      ? messages.filter(
          (message) =>
            !(
              message.role.toLowerCase() === "user" &&
              shouldHideCodexMessageFromToc(message.content)
            ),
        )
      : messages;
    const markdown = formatSessionMarkdown(exportMessages);
    if (!markdown) {
      toast.error(t("sessionManager.exportEmpty"));
      return;
    }

    setIsExporting(true);
    try {
      const destination = await sessionsApi.exportMarkdown(
        getSessionMarkdownFileName(selectedSession),
        markdown,
      );
      if (destination) {
        toast.success(t("sessionManager.exportSuccess"), { description: destination });
      }
    } catch (error) {
      toast.error(
        t("sessionManager.exportFailed", { error: extractErrorMessage(error) }),
      );
    } finally {
      setIsExporting(false);
    }
  };

  const scrollToMessage = (index: number) => {
    messageRefs.current.get(index)?.scrollIntoView({
      behavior: "smooth",
      block: "center",
    });
    setActiveMessageIndex(index);
    setTocDialogOpen(false);
    window.setTimeout(() => setActiveMessageIndex(null), 2000);
  };

  const renderSessionItem = (session: SessionMeta) => (
    <SessionItem
      key={getSessionKey(session)}
      session={session}
      isSelected={selectedKey === getSessionKey(session)}
      selectionMode={false}
      isChecked={false}
      searchQuery={search}
      onSelect={setSelectedKey}
      onToggleChecked={() => undefined}
    />
  );

  return (
    <TooltipProvider>
      <div className="mx-auto flex h-full min-h-0 w-full max-w-[1600px] flex-col px-4 py-4 sm:px-6">
        <div className="grid min-h-0 flex-1 gap-4 md:grid-cols-[340px_minmax(0,1fr)]">
          <Card className="flex min-h-0 flex-col overflow-hidden">
            <CardHeader className="gap-3 border-b px-3 py-3">
              <div className="flex items-center justify-between gap-2">
                <div className="flex min-w-0 items-center gap-2">
                  <CardTitle className="text-sm">{t("sessionManager.sessionList")}</CardTitle>
                  <Badge variant="secondary">{filteredSessions.length}</Badge>
                </div>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label={t("common.refresh")}
                      onClick={() => void refetch()}
                      disabled={isFetching}
                    >
                      <RefreshCw className={isFetching ? "size-4 animate-spin" : "size-4"} />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{t("common.refresh")}</TooltipContent>
                </Tooltip>
              </div>
              <div className="relative">
                <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
                <Input
                  value={search}
                  onChange={(event) => setSearch(event.target.value)}
                  placeholder={t("sessionManager.searchPlaceholder")}
                  className="h-8 pl-8 text-sm"
                />
              </div>
              <div className="grid grid-cols-2 gap-2">
                <Select
                  value={providerFilter}
                  onValueChange={(value) => setProviderFilter(value as ProviderFilter)}
                >
                  <SelectTrigger aria-label={t("sessionManager.providerFilterTooltip")} className="h-8 text-xs">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {PROVIDER_FILTERS.map((provider) => (
                      <SelectItem key={provider} value={provider}>
                        <div className="flex items-center gap-2">
                          <SessionProviderIcon providerId={provider} size={14} />
                          <span>
                            {provider === "all"
                              ? t("sessionManager.providerFilterAll")
                              : getProviderLabel(provider, t)}
                          </span>
                        </div>
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Select
                  value={listViewMode}
                  onValueChange={(value) => setListViewMode(value as SessionListViewMode)}
                >
                  <SelectTrigger aria-label={t("sessionManager.viewModeTooltip")} className="h-8 text-xs">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="flat">
                      <div className="flex items-center gap-2"><List className="size-3.5" />{t("sessionManager.viewModeFlat")}</div>
                    </SelectItem>
                    <SelectItem value="grouped">
                      <div className="flex items-center gap-2"><ListTree className="size-3.5" />{t("sessionManager.viewModeGrouped")}</div>
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </CardHeader>
            <ScrollArea className="min-h-0 flex-1">
              <div className="space-y-1 p-2">
                {isLoading ? (
                  <p className="p-4 text-center text-sm text-muted-foreground">{t("sessionManager.loadingSessions")}</p>
                ) : filteredSessions.length === 0 ? (
                  <p className="p-4 text-center text-sm text-muted-foreground">{t("sessionManager.noSessions")}</p>
                ) : listViewMode === "flat" ? (
                  filteredSessions.map(renderSessionItem)
                ) : (
                  groupedSessions.map((providerGroup) => (
                    <section key={providerGroup.providerId} className="space-y-2 py-1">
                      <div className="flex items-center gap-2 px-2 pt-2 text-xs font-medium text-muted-foreground">
                        <SessionProviderIcon providerId={providerGroup.providerId} size={15} />
                        <span>{getProviderLabel(providerGroup.providerId, t)}</span>
                        <Badge variant="outline" className="ml-auto text-[10px]">{providerGroup.sessions.length}</Badge>
                      </div>
                      {providerGroup.directories.map((directory) => (
                        <div key={directory.key} className="space-y-1">
                          <div className="px-2 text-[11px] text-muted-foreground">{directory.label}</div>
                          {directory.sessions.map(renderSessionItem)}
                        </div>
                      ))}
                    </section>
                  ))
                )}
              </div>
            </ScrollArea>
          </Card>

          <Card className="flex min-h-0 flex-col overflow-hidden">
            {!selectedSession ? (
              <div className="flex flex-1 items-center justify-center p-6 text-sm text-muted-foreground">
                {t("sessionManager.selectSession")}
              </div>
            ) : (
              <>
                <CardHeader className="flex-row items-start justify-between gap-3 border-b px-4 py-3">
                  <div className="min-w-0 space-y-1">
                    <div className="flex items-center gap-2">
                      <SessionProviderIcon providerId={selectedSession.providerId} size={18} />
                      <CardTitle className="truncate text-base">{formatSessionTitle(selectedSession)}</CardTitle>
                    </div>
                    <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground">
                      {selectedSession.lastActiveAt && (
                        <span className="inline-flex items-center gap-1"><Clock className="size-3" />{formatTimestamp(selectedSession.lastActiveAt)}</span>
                      )}
                      {selectedSession.projectDir && (
                        <button
                          type="button"
                          className="inline-flex max-w-full items-center gap-1 truncate hover:text-foreground"
                          onClick={() => void copyText(selectedSession.projectDir!, t("sessionManager.projectDirCopied"))}
                        >
                          <FolderOpen className="size-3 shrink-0" />{selectedSession.projectDir}
                        </button>
                      )}
                    </div>
                  </div>
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        size="icon"
                        aria-label={t("sessionManager.export")}
                        onClick={() => void handleExportMarkdown()}
                        disabled={!hasExportableMessages || isExporting}
                      >
                        <Download className={isExporting ? "size-4 animate-pulse" : "size-4"} />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>{t("sessionManager.exportTooltip")}</TooltipContent>
                  </Tooltip>
                </CardHeader>
                <CardContent className="flex min-h-0 flex-1 p-0">
                  <ScrollArea className="min-w-0 flex-1">
                    <div className="space-y-3 p-4">
                      {isLoadingMessages ? (
                        <p className="p-4 text-center text-sm text-muted-foreground">{t("sessionManager.loadingMessages")}</p>
                      ) : messages.length === 0 ? (
                        <p className="p-4 text-center text-sm text-muted-foreground">{t("sessionManager.emptySession")}</p>
                      ) : (
                        messages.map((message, index) => (
                          <div key={`${message.ts ?? ""}-${index}`} ref={(node) => {
                            if (node) messageRefs.current.set(index, node);
                            else messageRefs.current.delete(index);
                          }}>
                            <SessionMessageItem
                              message={message}
                              isActive={activeMessageIndex === index}
                              searchQuery={search}
                              onCopy={(content) => void copyText(content, t("sessionManager.messageCopied"))}
                            />
                          </div>
                        ))
                      )}
                    </div>
                  </ScrollArea>
                  <SessionTocSidebar items={tocItems} onItemClick={scrollToMessage} />
                </CardContent>
                <SessionTocDialog
                  items={tocItems}
                  onItemClick={scrollToMessage}
                  open={tocDialogOpen}
                  onOpenChange={setTocDialogOpen}
                />
              </>
            )}
          </Card>
        </div>
      </div>
    </TooltipProvider>
  );
}
