import { useState } from "react";
import { ChevronDown, ChevronUp, MessageSquareText } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type {
  SessionMessageKind,
  SessionSearchHit,
  SessionSearchResponse,
  SessionSearchResult,
} from "@/types";
import { SessionProviderIcon } from "./SessionProviderIcon";
import {
  formatRelativeTime,
  formatSessionTitle,
  getProviderLabel,
  highlightTerms,
} from "./utils";

interface AdvancedSessionSearchResultsProps {
  response?: SessionSearchResponse;
  terms: string[];
  isSearching: boolean;
  error?: string;
  onSelectHit: (result: SessionSearchResult, hit: SessionSearchHit) => void;
  onSelectSession: (result: SessionSearchResult) => void;
}

const kindTranslationKey: Record<SessionMessageKind, string> = {
  text: "sessionManager.advanced.kindText",
  reasoning: "sessionManager.thinking",
  toolCall: "sessionManager.toolCall",
  toolResult: "sessionManager.toolResult",
};

export function AdvancedSessionSearchResults({
  response,
  terms,
  isSearching,
  error,
  onSelectHit,
  onSelectSession,
}: AdvancedSessionSearchResultsProps) {
  const { t } = useTranslation();
  const [expandedSessions, setExpandedSessions] = useState<Set<string>>(
    () => new Set(),
  );

  if (isSearching) {
    return (
      <p className="p-4 text-center text-sm text-muted-foreground">
        {t("sessionManager.advanced.searching")}
      </p>
    );
  }

  if (error) {
    return <p className="p-4 text-center text-sm text-destructive">{error}</p>;
  }

  if (!response || response.results.length === 0) {
    return (
      <p className="p-4 text-center text-sm text-muted-foreground">
        {t("sessionManager.advanced.noResults")}
      </p>
    );
  }

  return (
    <div className="grid gap-2 p-2">
      <div className="flex items-center gap-2 px-1 text-xs text-muted-foreground">
        <Badge variant="secondary" className="text-[11px]">
          {t("sessionManager.advanced.resultCount", {
            count: response.totalSessions,
          })}
        </Badge>
        <span className="truncate">
          {t("sessionManager.advanced.matchCount", {
            count: response.totalMatches,
          })}
        </span>
      </div>

      {response.truncated && (
        <p className="px-1 text-[11px] text-muted-foreground">
          {t("sessionManager.advanced.truncated")}
        </p>
      )}
      {response.failedSessions > 0 && (
        <p className="px-1 text-[11px] text-amber-600 dark:text-amber-400">
          {t("sessionManager.advanced.failedSessions", {
            count: response.failedSessions,
          })}
        </p>
      )}

      {response.results.map((result) => {
        const sessionKey = `${result.session.providerId}:${result.session.sessionId}:${result.session.sourcePath ?? ""}`;
        const expanded = expandedSessions.has(sessionKey);
        const visibleHits = expanded ? result.hits : result.hits.slice(0, 3);
        const hiddenHitCount = Math.max(
          result.hits.length - visibleHits.length,
          0,
        );

        return (
          <div
            key={sessionKey}
            className="overflow-hidden rounded-md border bg-card"
          >
            <button
              type="button"
              className="flex w-full items-start gap-2 px-2.5 py-2 text-left transition-colors hover:bg-muted/50"
              onClick={() => onSelectSession(result)}
            >
              <SessionProviderIcon
                providerId={result.session.providerId}
                size={16}
              />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-sm font-medium">
                  {formatSessionTitle(result.session)}
                </span>
                <span className="mt-0.5 flex min-w-0 items-center gap-1.5 text-[11px] text-muted-foreground">
                  <span className="truncate">
                    {getProviderLabel(result.session.providerId, t)}
                  </span>
                  <span>·</span>
                  <span className="truncate">
                    {result.session.projectDir ||
                      t("sessionManager.unknownDirectory")}
                  </span>
                  <span>·</span>
                  <span className="shrink-0">
                    {formatRelativeTime(
                      result.session.lastActiveAt || result.session.createdAt,
                      t,
                    )}
                  </span>
                </span>
              </span>
              <Badge variant="outline" className="shrink-0 text-[10px]">
                {result.totalMatches}
              </Badge>
            </button>

            {visibleHits.length > 0 && (
              <div className="border-t border-border/60 bg-muted/15">
                {visibleHits.map((hit, index) => {
                  const metadataHit = hit.field !== "message";
                  const kindLabel = hit.kind
                    ? t(kindTranslationKey[hit.kind])
                    : null;
                  const hitLabel = metadataHit
                    ? t(`sessionManager.advanced.field.${hit.field}`)
                    : [hit.role, kindLabel].filter(Boolean).join(" · ");

                  return (
                    <button
                      key={`${hit.field}:${hit.messageIndex ?? "meta"}:${index}`}
                      type="button"
                      className={cn(
                        "group flex w-full gap-2 border-b border-border/40 px-2.5 py-2 text-left last:border-b-0 hover:bg-muted/60",
                        !metadataHit && "pl-8",
                      )}
                      onClick={() => onSelectHit(result, hit)}
                    >
                      <MessageSquareText className="mt-0.5 size-3 shrink-0 text-muted-foreground/70" />
                      <span className="min-w-0 flex-1">
                        <span className="mb-0.5 block text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
                          {hitLabel}
                        </span>
                        <span className="line-clamp-3 block text-xs leading-relaxed text-foreground/85">
                          {highlightTerms(hit.snippet, terms)}
                        </span>
                      </span>
                    </button>
                  );
                })}

                {result.hits.length > 3 && (
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-7 w-full gap-1 rounded-none text-[11px] text-muted-foreground"
                    onClick={() => {
                      setExpandedSessions((current) => {
                        const next = new Set(current);
                        if (expanded) next.delete(sessionKey);
                        else next.add(sessionKey);
                        return next;
                      });
                    }}
                  >
                    {expanded ? (
                      <>
                        <ChevronUp className="size-3" />
                        {t("sessionManager.advanced.collapseMatches")}
                      </>
                    ) : (
                      <>
                        <ChevronDown className="size-3" />
                        {t("sessionManager.advanced.moreMatches", {
                          count: hiddenHitCount,
                        })}
                      </>
                    )}
                  </Button>
                )}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}
