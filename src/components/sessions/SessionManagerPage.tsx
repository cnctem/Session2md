import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  ChevronDown,
  ChevronRight,
  ChevronsDownUp,
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
import {
  useSessionMessagesQuery,
  useSessionsQuery,
} from "@/lib/query/sessions";
import { sessionsApi } from "@/lib/api/sessions";
import { extractErrorMessage } from "@/utils/errorUtils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
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
  getSessionDirectoryGroupKey,
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

type SessionGroupExpansionState = {
  expandedProviderIds: Set<string>;
  expandedDirectoryKeys: Set<string>;
};

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
const GROUP_EXPANSION_STORAGE_KEY =
  "session2md.sessionManager.groupExpansionState";

const readInitialListViewMode = (): SessionListViewMode => {
  if (typeof window === "undefined") return "flat";
  const stored = window.localStorage.getItem(LIST_VIEW_MODE_STORAGE_KEY);
  return stored === "grouped" ? "grouped" : "flat";
};

const readInitialGroupExpansionState = (): SessionGroupExpansionState => {
  if (typeof window === "undefined") {
    return {
      expandedProviderIds: new Set(),
      expandedDirectoryKeys: new Set(),
    };
  }

  try {
    const stored = window.localStorage.getItem(GROUP_EXPANSION_STORAGE_KEY);
    const parsed = stored ? JSON.parse(stored) : null;
    if (!parsed || typeof parsed !== "object") {
      return {
        expandedProviderIds: new Set(),
        expandedDirectoryKeys: new Set(),
      };
    }

    const toStringSet = (value: unknown) =>
      new Set(
        Array.isArray(value)
          ? value.filter((entry): entry is string => typeof entry === "string")
          : [],
      );

    return {
      expandedProviderIds: toStringSet(parsed.expandedProviderIds),
      expandedDirectoryKeys: toStringSet(parsed.expandedDirectoryKeys),
    };
  } catch {
    return {
      expandedProviderIds: new Set(),
      expandedDirectoryKeys: new Set(),
    };
  }
};

const serializeGroupExpansionState = (
  expandedProviderIds: Set<string>,
  expandedDirectoryKeys: Set<string>,
) =>
  JSON.stringify({
    expandedProviderIds: Array.from(expandedProviderIds).sort(),
    expandedDirectoryKeys: Array.from(expandedDirectoryKeys).sort(),
  });

const filterSetToAllowedValues = (
  current: Set<string>,
  allowedValues: Set<string>,
) => {
  const next = new Set(
    Array.from(current).filter((value) => allowedValues.has(value)),
  );
  return next.size === current.size ? current : next;
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
  const [initialGroupExpansionState] = useState(readInitialGroupExpansionState);
  const [expandedProviderGroups, setExpandedProviderGroups] = useState<
    Set<string>
  >(() => initialGroupExpansionState.expandedProviderIds);
  const [expandedDirectoryGroups, setExpandedDirectoryGroups] = useState<
    Set<string>
  >(() => initialGroupExpansionState.expandedDirectoryKeys);
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
  const validGroupExpansionKeys = useMemo(
    () => ({
      providerIds: new Set(sessions.map((session) => session.providerId)),
      directoryKeys: new Set(
        sessions.map((session) =>
          getSessionDirectoryGroupKey(session.providerId, session.projectDir),
        ),
      ),
    }),
    [sessions],
  );

  useEffect(() => {
    window.localStorage.setItem(LIST_VIEW_MODE_STORAGE_KEY, listViewMode);
  }, [listViewMode]);

  useEffect(() => {
    window.localStorage.setItem(
      GROUP_EXPANSION_STORAGE_KEY,
      serializeGroupExpansionState(
        expandedProviderGroups,
        expandedDirectoryGroups,
      ),
    );
  }, [expandedDirectoryGroups, expandedProviderGroups]);

  useEffect(() => {
    if (isLoading) return;

    setExpandedProviderGroups((current) =>
      filterSetToAllowedValues(current, validGroupExpansionKeys.providerIds),
    );
    setExpandedDirectoryGroups((current) =>
      filterSetToAllowedValues(current, validGroupExpansionKeys.directoryKeys),
    );
  }, [isLoading, validGroupExpansionKeys]);

  useEffect(() => {
    if (filteredSessions.length === 0) {
      setSelectedKey(null);
      return;
    }
    if (
      !filteredSessions.some(
        (session) => getSessionKey(session) === selectedKey,
      )
    ) {
      setSelectedKey(getSessionKey(filteredSessions[0]));
    }
  }, [filteredSessions, selectedKey]);

  const selectedSession = useMemo(
    () =>
      filteredSessions.find(
        (session) => getSessionKey(session) === selectedKey,
      ) ?? null,
    [filteredSessions, selectedKey],
  );
  const { data: messages = [], isLoading: isLoadingMessages } =
    useSessionMessagesQuery(
      selectedSession?.providerId,
      selectedSession?.sourcePath,
    );
  const isCodexSession = selectedSession?.providerId === "codex";
  const exportMessages = useMemo(
    () =>
      isCodexSession
        ? messages.filter(
            (message) =>
              !(
                message.role.toLowerCase() === "user" &&
                shouldHideCodexMessageFromToc(message.content)
              ),
          )
        : messages,
    [isCodexSession, messages],
  );
  const sessionMarkdown = useMemo(
    () => formatSessionMarkdown(exportMessages),
    [exportMessages],
  );
  const hasExportableMessages = sessionMarkdown.length > 0;
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
            isCodexSession
              ? extractCodexPromptPreview(message.content)
              : message.content,
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
    if (!sessionMarkdown) {
      toast.error(t("sessionManager.exportEmpty"));
      return;
    }

    setIsExporting(true);
    try {
      const destination = await sessionsApi.exportMarkdown(
        getSessionMarkdownFileName(selectedSession),
        sessionMarkdown,
      );
      if (destination) {
        toast.success(t("sessionManager.exportSuccess"), {
          description: destination,
        });
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

  const setProviderGroupOpen = (providerId: string, open: boolean) => {
    setExpandedProviderGroups((current) => {
      const next = new Set(current);
      if (open) next.add(providerId);
      else next.delete(providerId);
      return next;
    });
  };

  const setDirectoryGroupOpen = (directoryKey: string, open: boolean) => {
    setExpandedDirectoryGroups((current) => {
      const next = new Set(current);
      if (open) next.add(directoryKey);
      else next.delete(directoryKey);
      return next;
    });
  };

  const handleCollapseAllGroups = () => {
    setExpandedProviderGroups(new Set());
    setExpandedDirectoryGroups(new Set());
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
                  <CardTitle className="text-sm">
                    {t("sessionManager.sessionList")}
                  </CardTitle>
                  <Badge variant="secondary">{filteredSessions.length}</Badge>
                </div>
                <div className="flex shrink-0 items-center gap-1">
                  {listViewMode === "grouped" && (
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          variant="ghost"
                          size="icon"
                          aria-label={t("sessionManager.collapseAllGroups")}
                          onClick={handleCollapseAllGroups}
                        >
                          <ChevronsDownUp className="size-4" />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>
                        {t("sessionManager.collapseAllGroups")}
                      </TooltipContent>
                    </Tooltip>
                  )}
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        variant="ghost"
                        size="icon"
                        aria-label={t("common.refresh")}
                        onClick={() => void refetch()}
                        disabled={isFetching}
                      >
                        <RefreshCw
                          className={
                            isFetching ? "size-4 animate-spin" : "size-4"
                          }
                        />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>{t("common.refresh")}</TooltipContent>
                  </Tooltip>
                </div>
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
                  onValueChange={(value) =>
                    setProviderFilter(value as ProviderFilter)
                  }
                >
                  <SelectTrigger
                    aria-label={t("sessionManager.providerFilterTooltip")}
                    className="h-8 text-xs"
                  >
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {PROVIDER_FILTERS.map((provider) => (
                      <SelectItem key={provider} value={provider}>
                        <div className="flex items-center gap-2">
                          <SessionProviderIcon
                            providerId={provider}
                            size={14}
                          />
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
                  onValueChange={(value) =>
                    setListViewMode(value as SessionListViewMode)
                  }
                >
                  <SelectTrigger
                    aria-label={t("sessionManager.viewModeTooltip")}
                    className="h-8 text-xs"
                  >
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="flat">
                      <div className="flex items-center gap-2">
                        <List className="size-3.5" />
                        {t("sessionManager.viewModeFlat")}
                      </div>
                    </SelectItem>
                    <SelectItem value="grouped">
                      <div className="flex items-center gap-2">
                        <ListTree className="size-3.5" />
                        {t("sessionManager.viewModeGrouped")}
                      </div>
                    </SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </CardHeader>
            <ScrollArea className="min-h-0 flex-1">
              <div className="space-y-1 p-2">
                {isLoading ? (
                  <p className="p-4 text-center text-sm text-muted-foreground">
                    {t("sessionManager.loadingSessions")}
                  </p>
                ) : filteredSessions.length === 0 ? (
                  <p className="p-4 text-center text-sm text-muted-foreground">
                    {t("sessionManager.noSessions")}
                  </p>
                ) : listViewMode === "flat" ? (
                  filteredSessions.map(renderSessionItem)
                ) : (
                  groupedSessions.map((providerGroup) => {
                    const providerOpen = expandedProviderGroups.has(
                      providerGroup.providerId,
                    );
                    const providerLabel = getProviderLabel(
                      providerGroup.providerId,
                      t,
                    );

                    return (
                      <Collapsible
                        key={providerGroup.providerId}
                        open={providerOpen}
                        onOpenChange={(open) =>
                          setProviderGroupOpen(providerGroup.providerId, open)
                        }
                      >
                        <div className="flex w-full items-center rounded-md border bg-muted/40 px-2.5 py-2 transition-colors hover:bg-muted">
                          <CollapsibleTrigger asChild>
                            <button
                              type="button"
                              className="flex min-w-0 flex-1 items-center gap-2 text-left"
                              aria-label={t(
                                "sessionManager.toggleProviderGroup",
                                {
                                  provider: providerLabel,
                                },
                              )}
                            >
                              {providerOpen ? (
                                <ChevronDown className="size-3.5 shrink-0 text-muted-foreground" />
                              ) : (
                                <ChevronRight className="size-3.5 shrink-0 text-muted-foreground" />
                              )}
                              <SessionProviderIcon
                                providerId={providerGroup.providerId}
                                size={16}
                              />
                              <span className="min-w-0 flex-1 truncate text-sm font-medium">
                                {providerLabel}
                              </span>
                              <Badge
                                variant="secondary"
                                className="text-[10px]"
                              >
                                {providerGroup.sessions.length}
                              </Badge>
                            </button>
                          </CollapsibleTrigger>
                        </div>
                        <CollapsibleContent className="mt-1 space-y-1 pl-2">
                          {providerGroup.directories.map((directory) => {
                            const directoryOpen = expandedDirectoryGroups.has(
                              directory.key,
                            );

                            return (
                              <Collapsible
                                key={directory.key}
                                open={directoryOpen}
                                onOpenChange={(open) =>
                                  setDirectoryGroupOpen(directory.key, open)
                                }
                              >
                                <div className="flex w-full items-center rounded-md px-2.5 py-1.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground">
                                  <CollapsibleTrigger asChild>
                                    <button
                                      type="button"
                                      className="flex min-w-0 flex-1 items-center gap-2 text-left"
                                      aria-label={t(
                                        "sessionManager.toggleDirectoryGroup",
                                        { directory: directory.label },
                                      )}
                                    >
                                      {directoryOpen ? (
                                        <ChevronDown className="size-3.5 shrink-0" />
                                      ) : (
                                        <ChevronRight className="size-3.5 shrink-0" />
                                      )}
                                      <FolderOpen className="size-3.5 shrink-0" />
                                      <Tooltip>
                                        <TooltipTrigger asChild>
                                          <span className="min-w-0 flex-1 truncate text-xs font-medium">
                                            {directory.label}
                                          </span>
                                        </TooltipTrigger>
                                        <TooltipContent
                                          side="bottom"
                                          className="max-w-xs"
                                        >
                                          <p className="break-all font-mono text-xs">
                                            {directory.projectDir ??
                                              t(
                                                "sessionManager.unknownDirectory",
                                              )}
                                          </p>
                                        </TooltipContent>
                                      </Tooltip>
                                      <Badge
                                        variant="outline"
                                        className="text-[10px]"
                                      >
                                        {directory.sessions.length}
                                      </Badge>
                                    </button>
                                  </CollapsibleTrigger>
                                </div>
                                <CollapsibleContent className="mt-1 space-y-1 pl-3">
                                  {directory.sessions.map(renderSessionItem)}
                                </CollapsibleContent>
                              </Collapsible>
                            );
                          })}
                        </CollapsibleContent>
                      </Collapsible>
                    );
                  })
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
                      <SessionProviderIcon
                        providerId={selectedSession.providerId}
                        size={18}
                      />
                      <CardTitle className="truncate text-base">
                        {formatSessionTitle(selectedSession)}
                      </CardTitle>
                    </div>
                    <div className="flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground">
                      {selectedSession.lastActiveAt && (
                        <span className="inline-flex items-center gap-1">
                          <Clock className="size-3" />
                          {formatTimestamp(selectedSession.lastActiveAt)}
                        </span>
                      )}
                      {selectedSession.projectDir && (
                        <button
                          type="button"
                          className="inline-flex max-w-full items-center gap-1 truncate hover:text-foreground"
                          onClick={() =>
                            void copyText(
                              selectedSession.projectDir!,
                              t("sessionManager.projectDirCopied"),
                            )
                          }
                        >
                          <FolderOpen className="size-3 shrink-0" />
                          {selectedSession.projectDir}
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
                        <Download
                          className={
                            isExporting ? "size-4 animate-pulse" : "size-4"
                          }
                        />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>
                      {t("sessionManager.exportTooltip")}
                    </TooltipContent>
                  </Tooltip>
                </CardHeader>
                <CardContent className="flex min-h-0 flex-1 p-0">
                  <ScrollArea className="min-w-0 flex-1">
                    <div className="space-y-3 p-4">
                      {isLoadingMessages ? (
                        <p className="p-4 text-center text-sm text-muted-foreground">
                          {t("sessionManager.loadingMessages")}
                        </p>
                      ) : messages.length === 0 ? (
                        <p className="p-4 text-center text-sm text-muted-foreground">
                          {t("sessionManager.emptySession")}
                        </p>
                      ) : (
                        messages.map((message, index) => (
                          <div
                            key={`${message.ts ?? ""}-${index}`}
                            ref={(node) => {
                              if (node) messageRefs.current.set(index, node);
                              else messageRefs.current.delete(index);
                            }}
                          >
                            <SessionMessageItem
                              message={message}
                              isActive={activeMessageIndex === index}
                              searchQuery={search}
                              onCopy={(content) =>
                                void copyText(
                                  content,
                                  t("sessionManager.messageCopied"),
                                )
                              }
                            />
                          </div>
                        ))
                      )}
                    </div>
                  </ScrollArea>
                  <SessionTocSidebar
                    items={tocItems}
                    onItemClick={scrollToMessage}
                  />
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
