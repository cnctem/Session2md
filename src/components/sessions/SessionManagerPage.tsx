import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  ChevronDown,
  ChevronRight,
  ChevronsDownUp,
  Copy,
  Download,
  FileText,
  FolderOpen,
  List,
  ListTree,
  RefreshCw,
  Search,
  Clock,
  CheckSquare,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import {
  observeElementRect,
  useVirtualizer,
  type Rect,
  type Virtualizer,
} from "@tanstack/react-virtual";
import { useSessionSearch } from "@/hooks/useSessionSearch";
import {
  useSessionMessagesQuery,
  useSessionsQuery,
} from "@/lib/query/sessions";
import { useSession2mdSettingsQuery } from "@/lib/query/session2mdSettings";
import { sessionsApi } from "@/lib/api/sessions";
import { extractErrorMessage } from "@/utils/errorUtils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { ConfirmDialog } from "@/components/ConfirmDialog";
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
import {
  SESSION_PROVIDER_IDS,
  type SessionProviderId,
} from "@/lib/sessionProviders";
import { SessionItem } from "./SessionItem";
import { SessionMessageItem } from "./SessionMessageItem";
import { SessionProviderIcon } from "./SessionProviderIcon";
import { SessionTocDialog, SessionTocSidebar } from "./SessionToc";
import { groupSessionMessages } from "./messageGroups";
import {
  extractCodexPromptPreview,
  formatSessionGroupsMarkdown,
  formatSessionMessagePreview,
  formatSessionTitle,
  formatTimestamp,
  getBaseName,
  getSessionDirectoryGroupKey,
  getProviderLabel,
  getSessionMarkdownFileName,
  getSessionKey,
  groupSessionsByProviderAndDirectory,
  shouldHideCodexMessageFromToc,
  type SessionDirectoryGroup,
  type SessionProviderGroup,
} from "./utils";

type ProviderFilter = "all" | SessionProviderId;

type SessionListViewMode = "flat" | "grouped";

type SessionGroupExpansionState = {
  expandedProviderIds: Set<string>;
  expandedDirectoryKeys: Set<string>;
};

type GroupSelectionState = {
  checked: boolean | "indeterminate";
  isSelected: boolean;
  selectedCount: number;
  selectableCount: number;
};

type SessionListRow =
  | {
      kind: "provider";
      key: string;
      group: SessionProviderGroup;
    }
  | {
      kind: "directory";
      key: string;
      directory: SessionDirectoryGroup;
    }
  | {
      kind: "session";
      key: string;
      session: SessionMeta;
      indent: number;
    };

const isDeletableSession = (session: SessionMeta) =>
  Boolean(session.sourcePath) && session.canDelete !== false;

const observeElementRectWithFallback = <T extends Element>(
  instance: Virtualizer<T, Element>,
  callback: (rect: Rect) => void,
) =>
  observeElementRect(instance, (rect) =>
    callback({
      width: rect.width || 340,
      height: rect.height || 768,
    }),
  );

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
  const queryClient = useQueryClient();
  const { data, isLoading, isFetching, refetch } = useSessionsQuery();
  const { data: sessionSettings } = useSession2mdSettingsQuery();
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
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedSessionKeys, setSelectedSessionKeys] = useState<Set<string>>(
    () => new Set(),
  );
  const [deleteTargets, setDeleteTargets] = useState<SessionMeta[] | null>(
    null,
  );
  const [isDeleting, setIsDeleting] = useState(false);
  const [tocDialogOpen, setTocDialogOpen] = useState(false);
  const [activeMessageIndex, setActiveMessageIndex] = useState<number | null>(
    null,
  );
  const [expandedBlockOverrides, setExpandedBlockOverrides] = useState<
    ReadonlyMap<string, boolean>
  >(() => new Map());
  const sessionListScrollRef = useRef<HTMLDivElement>(null);
  const messageListScrollRef = useRef<HTMLDivElement>(null);
  const activeMessageTimeoutRef = useRef<number | null>(null);

  const toggleMessageBlock = useCallback(
    (blockKey: string, expanded: boolean) => {
      setExpandedBlockOverrides((current) => {
        const next = new Map(current);
        next.set(blockKey, expanded);
        return next;
      });
    },
    [],
  );

  useEffect(
    () => () => {
      if (activeMessageTimeoutRef.current !== null) {
        window.clearTimeout(activeMessageTimeoutRef.current);
      }
    },
    [],
  );

  const hiddenProviderIds = useMemo(
    () => new Set<string>(sessionSettings?.hiddenProviders ?? []),
    [sessionSettings?.hiddenProviders],
  );
  const visibleSessions = useMemo(
    () =>
      sessions.filter((session) => !hiddenProviderIds.has(session.providerId)),
    [hiddenProviderIds, sessions],
  );
  const providerFilterOptions = useMemo<ProviderFilter[]>(
    () => [
      "all",
      ...SESSION_PROVIDER_IDS.filter(
        (providerId) => !hiddenProviderIds.has(providerId),
      ),
    ],
    [hiddenProviderIds],
  );

  const { search: searchSessions } = useSessionSearch({
    sessions: visibleSessions,
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
      providerIds: new Set(
        visibleSessions.map((session) => session.providerId),
      ),
      directoryKeys: new Set(
        visibleSessions.map((session) =>
          getSessionDirectoryGroupKey(session.providerId, session.projectDir),
        ),
      ),
    }),
    [visibleSessions],
  );

  const sessionListRows = useMemo<SessionListRow[]>(() => {
    if (listViewMode === "flat") {
      return filteredSessions.map((session) => ({
        kind: "session",
        key: getSessionKey(session),
        session,
        indent: 0,
      }));
    }

    const rows: SessionListRow[] = [];
    for (const providerGroup of groupedSessions) {
      rows.push({
        kind: "provider",
        key: `provider:${providerGroup.providerId}`,
        group: providerGroup,
      });
      if (!expandedProviderGroups.has(providerGroup.providerId)) continue;

      for (const directory of providerGroup.directories) {
        rows.push({
          kind: "directory",
          key: directory.key,
          directory,
        });
        if (!expandedDirectoryGroups.has(directory.key)) continue;
        rows.push(
          ...directory.sessions.map((session) => ({
            kind: "session" as const,
            key: getSessionKey(session),
            session,
            indent: 20,
          })),
        );
      }
    }
    return rows;
  }, [
    expandedDirectoryGroups,
    expandedProviderGroups,
    filteredSessions,
    groupedSessions,
    listViewMode,
  ]);

  const getSessionListScrollElement = useCallback(
    () =>
      sessionListScrollRef.current?.closest<HTMLElement>(
        "[data-radix-scroll-area-viewport]",
      ) ?? null,
    [],
  );
  const sessionListVirtualizer = useVirtualizer({
    count: sessionListRows.length,
    getScrollElement: getSessionListScrollElement,
    observeElementRect: observeElementRectWithFallback,
    initialRect: { width: 340, height: 768 },
    estimateSize: (index) => {
      const row = sessionListRows[index];
      if (row?.kind === "provider") return 44;
      if (row?.kind === "directory") return 36;
      return 76;
    },
    getItemKey: (index) => sessionListRows[index]?.key ?? index,
    overscan: 8,
    gap: 4,
    paddingStart: 8,
    paddingEnd: 8,
  });

  useEffect(() => {
    if (providerFilter !== "all" && hiddenProviderIds.has(providerFilter)) {
      setProviderFilter("all");
    }
  }, [hiddenProviderIds, providerFilter]);

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

  useEffect(() => {
    const validKeys = new Set(
      visibleSessions.map((session) => getSessionKey(session)),
    );
    setSelectedSessionKeys((current) => {
      const next = new Set(
        Array.from(current).filter((key) => validKeys.has(key)),
      );
      return next.size === current.size ? current : next;
    });
  }, [visibleSessions]);

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
  const messageGroups = useMemo(
    () => groupSessionMessages(messages),
    [messages],
  );
  const getMessageListScrollElement = useCallback(
    () =>
      messageListScrollRef.current?.closest<HTMLElement>(
        "[data-radix-scroll-area-viewport]",
      ) ?? null,
    [],
  );
  const messageVirtualizer = useVirtualizer({
    count: messageGroups.length,
    getScrollElement: getMessageListScrollElement,
    observeElementRect: observeElementRectWithFallback,
    initialRect: { width: 1024, height: 768 },
    estimateSize: () => 140,
    getItemKey: (index) => messageGroups[index]?.id ?? index,
    overscan: 8,
    gap: 12,
    paddingStart: 16,
    paddingEnd: 16,
  });

  useEffect(() => {
    setExpandedBlockOverrides(new Map());
    const scrollElement = messageListScrollRef.current?.closest<HTMLElement>(
      "[data-radix-scroll-area-viewport]",
    );
    if (scrollElement) scrollElement.scrollTop = 0;
  }, [selectedSession?.providerId, selectedSession?.sourcePath]);
  const exportGroups = useMemo(
    () =>
      isCodexSession
        ? messageGroups.filter(
            (group) =>
              !(
                group.role.toLowerCase() === "user" &&
                shouldHideCodexMessageFromToc(group.content)
              ),
          )
        : messageGroups,
    [isCodexSession, messageGroups],
  );
  const exportOptions = useMemo(
    () => ({
      includeThinking: sessionSettings?.exportThinking ?? false,
      includeToolInputs: sessionSettings?.exportToolInputs ?? false,
      includeToolOutputs: sessionSettings?.exportToolOutputs ?? false,
    }),
    [
      sessionSettings?.exportThinking,
      sessionSettings?.exportToolInputs,
      sessionSettings?.exportToolOutputs,
    ],
  );
  const hasExportableMessages = useMemo(
    () =>
      exportGroups.some((group) => {
        const role = group.role.toLowerCase();
        if (role === "tool") {
          return Boolean(
            (exportOptions.includeToolInputs && group.toolInput?.trim()) ||
              (exportOptions.includeToolOutputs && group.toolOutput?.trim()),
          );
        }
        if (role !== "user" && role !== "assistant") return false;
        return Boolean(
          group.content.trim() ||
            (exportOptions.includeThinking && group.reasoning.trim()),
        );
      }),
    [exportGroups, exportOptions],
  );
  const tocItems = useMemo(
    () =>
      messageGroups
        .map((group, index) => ({ group, index }))
        .filter(({ group }) => {
          return (
            group.role.toLowerCase() === "user" &&
            !(isCodexSession && shouldHideCodexMessageFromToc(group.content))
          );
        })
        .map(({ group, index }) => ({
          index,
          preview: formatSessionMessagePreview(
            isCodexSession
              ? extractCodexPromptPreview(group.content)
              : group.content,
          ),
          ts: group.ts,
        })),
    [isCodexSession, messageGroups],
  );

  const copyText = useCallback(
    async (value: string, successMessage: string) => {
      try {
        await navigator.clipboard.writeText(value);
        toast.success(successMessage);
      } catch (error) {
        toast.error(extractErrorMessage(error) || t("common.error"));
      }
    },
    [t],
  );
  const handleCopyMessage = useCallback(
    (content: string) => {
      void copyText(content, t("sessionManager.messageCopied"));
    },
    [copyText, t],
  );
  const handleCopyCode = useCallback(
    (content: string) => {
      void copyText(content, t("sessionManager.codeCopied"));
    },
    [copyText, t],
  );
  const handleOpenLink = useCallback(
    (url: string) => {
      void sessionsApi.openExternalUrl(url).catch((error) => {
        toast.error(
          t("sessionManager.openLinkFailed", {
            error: extractErrorMessage(error),
            defaultValue: "Failed to open link: {{error}}",
          }),
        );
      });
    },
    [t],
  );

  const deletableFilteredSessions = useMemo(
    () => filteredSessions.filter(isDeletableSession),
    [filteredSessions],
  );
  const selectedDeletableSessions = useMemo(
    () =>
      visibleSessions.filter(
        (session) =>
          isDeletableSession(session) &&
          selectedSessionKeys.has(getSessionKey(session)),
      ),
    [selectedSessionKeys, visibleSessions],
  );
  const allFilteredSelected =
    deletableFilteredSessions.length > 0 &&
    deletableFilteredSessions.every((session) =>
      selectedSessionKeys.has(getSessionKey(session)),
    );

  useEffect(() => {
    if (!selectionMode) return;
    const visibleKeys = new Set(
      deletableFilteredSessions.map((session) => getSessionKey(session)),
    );
    setSelectedSessionKeys((current) => {
      const next = new Set(
        Array.from(current).filter((key) => visibleKeys.has(key)),
      );
      return next.size === current.size ? current : next;
    });
  }, [deletableFilteredSessions, selectionMode]);

  const toggleSessionChecked = (session: SessionMeta, checked: boolean) => {
    if (!isDeletableSession(session)) return;
    const key = getSessionKey(session);
    setSelectedSessionKeys((current) => {
      const next = new Set(current);
      if (checked) next.add(key);
      else next.delete(key);
      return next;
    });
  };

  const getGroupSelectionState = (
    groupSessions: SessionMeta[],
  ): GroupSelectionState => {
    const selectableSessions = groupSessions.filter(isDeletableSession);
    const selectedCount = selectableSessions.filter((session) =>
      selectedSessionKeys.has(getSessionKey(session)),
    ).length;
    const isSelected =
      selectableSessions.length > 0 &&
      selectedCount === selectableSessions.length;
    return {
      checked:
        selectedCount === 0 ? false : isSelected ? true : "indeterminate",
      isSelected,
      selectedCount,
      selectableCount: selectableSessions.length,
    };
  };

  const toggleSessionGroupChecked = (
    groupSessions: SessionMeta[],
    checked: boolean,
  ) => {
    setSelectedSessionKeys((current) => {
      const next = new Set(current);
      groupSessions.forEach((session) => {
        if (!isDeletableSession(session)) return;
        const key = getSessionKey(session);
        if (checked) next.add(key);
        else next.delete(key);
      });
      return next;
    });
  };

  const handleToggleSelectAll = () => {
    setSelectedSessionKeys((current) => {
      const next = new Set(current);
      deletableFilteredSessions.forEach((session) => {
        const key = getSessionKey(session);
        if (allFilteredSelected) next.delete(key);
        else next.add(key);
      });
      return next;
    });
  };

  const exitSelectionMode = () => {
    setSelectionMode(false);
    setSelectedSessionKeys(new Set());
  };

  const removeDeletedSessions = (deleted: SessionMeta[]) => {
    const deletedKeys = new Set(
      deleted.map((session) => getSessionKey(session)),
    );
    queryClient.setQueryData<SessionMeta[]>(["sessions"], (current) =>
      (current ?? []).filter(
        (session) => !deletedKeys.has(getSessionKey(session)),
      ),
    );
    deleted.forEach((session) => {
      queryClient.removeQueries({
        queryKey: ["sessionMessages", session.providerId, session.sourcePath],
      });
    });
    setSelectedSessionKeys((current) => {
      const next = new Set(current);
      deletedKeys.forEach((key) => next.delete(key));
      return next;
    });
    setSelectedKey((current) =>
      current && deletedKeys.has(current) ? null : current,
    );
  };

  const handleDeleteConfirm = async () => {
    if (!deleteTargets?.length || isDeleting) return;
    const targets = deleteTargets.filter(
      (session): session is SessionMeta & { sourcePath: string } =>
        isDeletableSession(session),
    );
    if (!targets.length) {
      setDeleteTargets(null);
      return;
    }

    setIsDeleting(true);
    try {
      if (targets.length === 1) {
        const [target] = targets;
        const deleted = await sessionsApi.delete({
          providerId: target.providerId,
          sessionId: target.sessionId,
          sourcePath: target.sourcePath,
        });
        if (!deleted) {
          throw new Error(t("sessionManager.sessionNotFound"));
        }
        removeDeletedSessions([target]);
        toast.success(t("sessionManager.sessionDeleted"));
      } else {
        const results = await sessionsApi.deleteMany(
          targets.map((session) => ({
            providerId: session.providerId,
            sessionId: session.sessionId,
            sourcePath: session.sourcePath,
          })),
        );
        const successfulKeys = new Set(
          results
            .filter((result) => result.success)
            .map((result) =>
              getSessionKey({
                providerId: result.providerId,
                sessionId: result.sessionId,
                sourcePath: result.sourcePath,
              }),
            ),
        );
        const successfulTargets = targets.filter((session) =>
          successfulKeys.has(getSessionKey(session)),
        );
        if (successfulTargets.length) {
          removeDeletedSessions(successfulTargets);
          toast.success(
            t("sessionManager.batchDeleteSuccess", {
              count: successfulTargets.length,
            }),
          );
        }
        const failed = results.filter((result) => !result.success);
        if (failed.length) {
          toast.error(
            t("sessionManager.batchDeleteFailed", { failed: failed.length }),
            { description: failed[0].error },
          );
        }
      }
      setDeleteTargets(null);
      await queryClient.invalidateQueries({ queryKey: ["sessions"] });
    } catch (error) {
      toast.error(
        targets.length === 1
          ? t("sessionManager.deleteFailed", {
              error: extractErrorMessage(error),
            })
          : extractErrorMessage(error) ||
              t("sessionManager.batchDeleteRequestFailed"),
      );
    } finally {
      setIsDeleting(false);
    }
  };

  const handleExportMarkdown = async () => {
    if (!selectedSession || isExporting) return;
    if (!hasExportableMessages) {
      toast.error(t("sessionManager.exportEmpty"));
      return;
    }

    setIsExporting(true);
    try {
      await new Promise<void>((resolve) => window.setTimeout(resolve, 0));
      const markdown = formatSessionGroupsMarkdown(exportGroups, exportOptions);
      if (!markdown) {
        toast.error(t("sessionManager.exportEmpty"));
        return;
      }

      const destination = await sessionsApi.exportMarkdown(
        getSessionMarkdownFileName(selectedSession),
        markdown,
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
    messageVirtualizer.scrollToIndex(index, {
      align: "center",
      behavior: "smooth",
    });
    setActiveMessageIndex(index);
    setTocDialogOpen(false);
    if (activeMessageTimeoutRef.current !== null) {
      window.clearTimeout(activeMessageTimeoutRef.current);
    }
    activeMessageTimeoutRef.current = window.setTimeout(() => {
      setActiveMessageIndex(null);
      activeMessageTimeoutRef.current = null;
    }, 2000);
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
      session={session}
      isSelected={selectedKey === getSessionKey(session)}
      selectionMode={selectionMode}
      isChecked={selectedSessionKeys.has(getSessionKey(session))}
      isCheckDisabled={!isDeletableSession(session) || isDeleting}
      searchQuery={search}
      onSelect={setSelectedKey}
      onToggleChecked={(checked) => toggleSessionChecked(session, checked)}
    />
  );

  const renderSessionListRow = (row: SessionListRow) => {
    if (row.kind === "session") {
      return (
        <div style={{ paddingLeft: row.indent }}>
          {renderSessionItem(row.session)}
        </div>
      );
    }

    if (row.kind === "provider") {
      const { group } = row;
      const providerOpen = expandedProviderGroups.has(group.providerId);
      const providerLabel = getProviderLabel(group.providerId, t);
      const providerSelection = getGroupSelectionState(group.sessions);

      return (
        <div className="flex w-full items-center rounded-md border bg-muted/40 px-2.5 py-2 transition-colors hover:bg-muted">
          {selectionMode && (
            <Checkbox
              className="mr-2 shrink-0"
              checked={providerSelection.checked}
              disabled={!providerSelection.selectableCount || isDeleting}
              aria-label={t("sessionManager.selectProviderGroupForBatch", {
                provider: providerLabel,
              })}
              onCheckedChange={(checked) =>
                toggleSessionGroupChecked(group.sessions, Boolean(checked))
              }
            />
          )}
          <button
            type="button"
            className="flex min-w-0 flex-1 items-center gap-2 text-left"
            aria-label={t("sessionManager.toggleProviderGroup", {
              provider: providerLabel,
            })}
            onClick={() =>
              setProviderGroupOpen(group.providerId, !providerOpen)
            }
          >
            {providerOpen ? (
              <ChevronDown className="size-3.5 shrink-0 text-muted-foreground" />
            ) : (
              <ChevronRight className="size-3.5 shrink-0 text-muted-foreground" />
            )}
            <SessionProviderIcon providerId={group.providerId} size={16} />
            <span className="min-w-0 flex-1 truncate text-sm font-medium">
              {providerLabel}
            </span>
            <Badge variant="secondary" className="text-[10px]">
              {group.sessions.length}
            </Badge>
          </button>
        </div>
      );
    }

    const { directory } = row;
    const directoryOpen = expandedDirectoryGroups.has(directory.key);
    const directorySelection = getGroupSelectionState(directory.sessions);

    return (
      <div
        className="flex w-full items-center rounded-md px-2.5 py-1.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        style={{ marginLeft: 8 }}
      >
        {selectionMode && (
          <Checkbox
            className="mr-2 shrink-0"
            checked={directorySelection.checked}
            disabled={!directorySelection.selectableCount || isDeleting}
            aria-label={t("sessionManager.selectDirectoryGroupForBatch", {
              directory: directory.label,
            })}
            onCheckedChange={(checked) =>
              toggleSessionGroupChecked(directory.sessions, Boolean(checked))
            }
          />
        )}
        <button
          type="button"
          className="flex min-w-0 flex-1 items-center gap-2 text-left"
          aria-label={t("sessionManager.toggleDirectoryGroup", {
            directory: directory.label,
          })}
          onClick={() => setDirectoryGroupOpen(directory.key, !directoryOpen)}
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
            <TooltipContent side="bottom" className="max-w-xs">
              <p className="break-all font-mono text-xs">
                {directory.projectDir ?? t("sessionManager.unknownDirectory")}
              </p>
            </TooltipContent>
          </Tooltip>
          <Badge variant="outline" className="text-[10px]">
            {directory.sessions.length}
          </Badge>
        </button>
      </div>
    );
  };

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
                  {(selectionMode || deletableFilteredSessions.length > 0) && (
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          variant={selectionMode ? "secondary" : "ghost"}
                          size="icon"
                          aria-label={
                            selectionMode
                              ? t("sessionManager.exitBatchModeTooltip")
                              : t("sessionManager.manageBatchTooltip")
                          }
                          onClick={() => {
                            if (selectionMode) exitSelectionMode();
                            else setSelectionMode(true);
                          }}
                          disabled={isDeleting}
                        >
                          <CheckSquare className="size-4" />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>
                        {selectionMode
                          ? t("sessionManager.exitBatchModeTooltip")
                          : t("sessionManager.manageBatchTooltip")}
                      </TooltipContent>
                    </Tooltip>
                  )}
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
                    {providerFilterOptions.map((provider) => (
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
              {selectionMode && (
                <div className="grid gap-2 rounded-md border bg-muted/40 px-2.5 py-2">
                  <div className="flex items-center gap-2 text-xs text-muted-foreground">
                    <Badge variant="outline" className="text-xs">
                      {t("sessionManager.selectedCount", {
                        count: selectedDeletableSessions.length,
                      })}
                    </Badge>
                    <span className="truncate">
                      {t("sessionManager.batchModeHint")}
                    </span>
                  </div>
                  <div className="flex flex-wrap items-center gap-2">
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-7 px-2 text-xs"
                      onClick={handleToggleSelectAll}
                      disabled={!deletableFilteredSessions.length || isDeleting}
                    >
                      {allFilteredSelected
                        ? t("sessionManager.clearFilteredSelection")
                        : t("sessionManager.selectAllFiltered")}
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="h-7 px-2 text-xs"
                      onClick={() => setSelectedSessionKeys(new Set())}
                      disabled={!selectedDeletableSessions.length || isDeleting}
                    >
                      {t("sessionManager.clearSelection")}
                    </Button>
                    <Button
                      variant="destructive"
                      size="sm"
                      className="ml-auto h-7 gap-1.5 px-2 text-xs"
                      onClick={() =>
                        setDeleteTargets(selectedDeletableSessions)
                      }
                      disabled={!selectedDeletableSessions.length || isDeleting}
                    >
                      <Trash2 className="size-3.5" />
                      {t("sessionManager.deleteSelected")}
                    </Button>
                  </div>
                </div>
              )}
            </CardHeader>
            <ScrollArea className="min-h-0 flex-1">
              {isLoading ? (
                <p className="p-4 text-center text-sm text-muted-foreground">
                  {t("sessionManager.loadingSessions")}
                </p>
              ) : filteredSessions.length === 0 ? (
                <p className="p-4 text-center text-sm text-muted-foreground">
                  {t("sessionManager.noSessions")}
                </p>
              ) : (
                <div
                  ref={sessionListScrollRef}
                  className="relative w-full"
                  style={{ height: sessionListVirtualizer.getTotalSize() }}
                >
                  {sessionListVirtualizer
                    .getVirtualItems()
                    .map((virtualRow) => {
                      const row = sessionListRows[virtualRow.index];
                      if (!row) return null;
                      return (
                        <div
                          key={virtualRow.key}
                          ref={sessionListVirtualizer.measureElement}
                          data-index={virtualRow.index}
                          data-session-row={row.kind}
                          className="absolute left-2 right-2 top-0"
                          style={{
                            transform: `translateY(${virtualRow.start}px)`,
                          }}
                        >
                          {renderSessionListRow(row)}
                        </div>
                      );
                    })}
                </div>
              )}
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
                  <div className="min-w-0 flex-1 space-y-2">
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
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <button
                              type="button"
                              aria-label={t("sessionManager.copyProjectDir")}
                              onClick={() =>
                                void copyText(
                                  selectedSession.projectDir!,
                                  t("sessionManager.projectDirCopied"),
                                )
                              }
                              className="flex items-center gap-1 transition-colors hover:text-foreground"
                            >
                              <FolderOpen className="size-3 shrink-0" />
                              <span className="max-w-[200px] truncate">
                                {getBaseName(selectedSession.projectDir)}
                              </span>
                            </button>
                          </TooltipTrigger>
                          <TooltipContent side="bottom" className="max-w-xs">
                            <p className="text-xs font-medium">
                              {t("sessionManager.copyProjectDir")}
                            </p>
                            <p className="break-all font-mono text-xs">
                              {selectedSession.projectDir}
                            </p>
                          </TooltipContent>
                        </Tooltip>
                      )}
                    </div>
                    {selectedSession.sourcePath && (
                      <div className="min-w-0 text-xs text-muted-foreground">
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <button
                              type="button"
                              aria-label={t("sessionManager.copySourcePath")}
                              onClick={() =>
                                void copyText(
                                  selectedSession.sourcePath!,
                                  t("sessionManager.sourcePathCopied"),
                                )
                              }
                              className="inline-flex max-w-full items-center gap-1 transition-colors hover:text-foreground"
                            >
                              <FileText className="size-3 shrink-0" />
                              <span className="max-w-[200px] truncate font-mono">
                                {getBaseName(selectedSession.sourcePath)}
                              </span>
                            </button>
                          </TooltipTrigger>
                          <TooltipContent side="bottom" className="max-w-xs">
                            <p className="break-all font-mono text-xs">
                              {selectedSession.sourcePath}
                            </p>
                            <p className="mt-1 text-muted-foreground">
                              {t("sessionManager.clickToCopyPath")}
                            </p>
                          </TooltipContent>
                        </Tooltip>
                      </div>
                    )}
                    {selectedSession.resumeCommand && (
                      <div className="flex min-w-0 items-center gap-2">
                        <div
                          className="min-w-0 flex-1 truncate rounded-md bg-muted/60 px-3 py-1.5 font-mono text-xs text-muted-foreground"
                          title={selectedSession.resumeCommand}
                        >
                          {selectedSession.resumeCommand}
                        </div>
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              type="button"
                              variant="ghost"
                              size="icon"
                              className="size-7 shrink-0"
                              aria-label={t("sessionManager.copyCommand")}
                              onClick={() =>
                                void copyText(
                                  selectedSession.resumeCommand!,
                                  t("sessionManager.resumeCommandCopied"),
                                )
                              }
                            >
                              <Copy className="size-3.5" />
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent>
                            {t("sessionManager.copyCommand")}
                          </TooltipContent>
                        </Tooltip>
                      </div>
                    )}
                  </div>
                  <div className="flex shrink-0 items-center gap-1">
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
                    {isDeletableSession(selectedSession) && (
                      <Tooltip>
                        <TooltipTrigger asChild>
                          <Button
                            variant="destructive"
                            size="icon"
                            aria-label={t("sessionManager.delete")}
                            onClick={() => setDeleteTargets([selectedSession])}
                            disabled={isDeleting}
                          >
                            <Trash2 className="size-4" />
                          </Button>
                        </TooltipTrigger>
                        <TooltipContent>
                          {t("sessionManager.deleteTooltip")}
                        </TooltipContent>
                      </Tooltip>
                    )}
                  </div>
                </CardHeader>
                <CardContent className="flex min-h-0 flex-1 p-0">
                  <ScrollArea className="min-w-0 flex-1">
                    {isLoadingMessages ? (
                      <p className="p-4 text-center text-sm text-muted-foreground">
                        {t("sessionManager.loadingMessages")}
                      </p>
                    ) : messageGroups.length === 0 ? (
                      <p className="p-4 text-center text-sm text-muted-foreground">
                        {t("sessionManager.emptySession")}
                      </p>
                    ) : (
                      <div
                        ref={messageListScrollRef}
                        className="relative w-full"
                        style={{ height: messageVirtualizer.getTotalSize() }}
                      >
                        {messageVirtualizer
                          .getVirtualItems()
                          .map((virtualMessage) => {
                            const group = messageGroups[virtualMessage.index];
                            if (!group) return null;
                            return (
                              <div
                                key={virtualMessage.key}
                                ref={messageVirtualizer.measureElement}
                                data-index={virtualMessage.index}
                                data-message-index={virtualMessage.index}
                                className="absolute left-4 right-4 top-0"
                                style={{
                                  transform: `translateY(${virtualMessage.start}px)`,
                                }}
                              >
                                <SessionMessageItem
                                  group={group}
                                  isActive={
                                    activeMessageIndex === virtualMessage.index
                                  }
                                  expandedBlockOverrides={
                                    expandedBlockOverrides
                                  }
                                  defaultExpandThinking={
                                    sessionSettings?.defaultExpandThinking ??
                                    false
                                  }
                                  defaultExpandTools={
                                    sessionSettings?.defaultExpandTools ?? false
                                  }
                                  defaultExpandSystem={
                                    sessionSettings?.defaultExpandSystem ??
                                    false
                                  }
                                  renderMarkdown={
                                    sessionSettings?.renderMarkdown ?? true
                                  }
                                  searchQuery={search}
                                  onCopy={handleCopyMessage}
                                  onCopyCode={handleCopyCode}
                                  onOpenLink={handleOpenLink}
                                  onToggleBlock={toggleMessageBlock}
                                />
                              </div>
                            );
                          })}
                      </div>
                    )}
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
      <ConfirmDialog
        isOpen={Boolean(deleteTargets)}
        title={
          deleteTargets && deleteTargets.length > 1
            ? t("sessionManager.batchDeleteConfirmTitle")
            : t("sessionManager.deleteConfirmTitle")
        }
        message={
          deleteTargets && deleteTargets.length > 1
            ? t("sessionManager.batchDeleteConfirmMessage", {
                count: deleteTargets.length,
              })
            : deleteTargets?.[0]
              ? t("sessionManager.deleteConfirmMessage", {
                  title: formatSessionTitle(deleteTargets[0]),
                  sessionId: deleteTargets[0].sessionId,
                })
              : ""
        }
        confirmText={
          deleteTargets && deleteTargets.length > 1
            ? t("sessionManager.batchDeleteConfirmAction")
            : t("sessionManager.deleteConfirmAction")
        }
        pending={isDeleting}
        onConfirm={() => void handleDeleteConfirm()}
        onCancel={() => {
          if (!isDeleting) setDeleteTargets(null);
        }}
      />
    </TooltipProvider>
  );
}
