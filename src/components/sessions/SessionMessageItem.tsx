import { memo } from "react";
import {
  Brain,
  ChevronDown,
  ChevronRight,
  ChevronUp,
  Copy,
  TerminalSquare,
} from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import {
  getMessageGroupCopyText,
  getToolInputDisplay,
  type SessionMessageGroup,
} from "./messageGroups";
import {
  formatTimestamp,
  getRoleLabel,
  getRoleTone,
  highlightText,
} from "./utils";

const COLLAPSE_THRESHOLD = 3000;
const COLLAPSED_LENGTH = 1500;

interface MessageTextBlockProps {
  blockKey: string;
  content: string;
  expandedBlockOverrides: ReadonlyMap<string, boolean>;
  onToggleBlock: (blockKey: string, expanded: boolean) => void;
  searchQuery?: string;
  className?: string;
}

function MessageTextBlock({
  blockKey,
  content,
  expandedBlockOverrides,
  onToggleBlock,
  searchQuery,
  className,
}: MessageTextBlockProps) {
  const { t } = useTranslation();
  const expanded = expandedBlockOverrides.get(blockKey) ?? false;
  const isLong = content.length > COLLAPSE_THRESHOLD;
  const hasSearchMatch =
    isLong &&
    !expanded &&
    !!searchQuery &&
    content.toLowerCase().includes(searchQuery.toLowerCase());
  const collapsed = isLong && !expanded && !hasSearchMatch;
  const displayContent = collapsed
    ? content.slice(0, COLLAPSED_LENGTH) + "…"
    : content;

  return (
    <>
      <div
        className={cn(
          "whitespace-pre-wrap break-words [overflow-wrap:anywhere] text-sm leading-relaxed min-w-0",
          className,
        )}
      >
        {searchQuery
          ? highlightText(displayContent, searchQuery)
          : displayContent}
      </div>
      {isLong && !hasSearchMatch && (
        <button
          type="button"
          aria-expanded={expanded}
          onClick={() => onToggleBlock(blockKey, !expanded)}
          className="mt-1.5 flex items-center gap-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
        >
          {expanded ? (
            <>
              <ChevronUp className="size-3" />
              {t("sessionManager.collapseContent", {
                defaultValue: "收起",
              })}
            </>
          ) : (
            <>
              <ChevronDown className="size-3" />
              {t("sessionManager.expandContent", {
                defaultValue: "展开完整内容",
              })}
              <span className="text-muted-foreground/60">
                ({Math.round(content.length / 1000)}k)
              </span>
            </>
          )}
        </button>
      )}
    </>
  );
}

const SectionDivider = () => <div className="my-3 border-t" />;

interface SessionMessageItemProps {
  group: SessionMessageGroup;
  isActive: boolean;
  expandedBlockOverrides: ReadonlyMap<string, boolean>;
  defaultExpandThinking: boolean;
  defaultExpandTools: boolean;
  defaultExpandSystem: boolean;
  searchQuery?: string;
  onCopy: (content: string) => void;
  onToggleBlock: (blockKey: string, expanded: boolean) => void;
}

export const SessionMessageItem = memo(function SessionMessageItem({
  group,
  isActive,
  expandedBlockOverrides,
  defaultExpandThinking,
  defaultExpandTools,
  defaultExpandSystem,
  searchQuery,
  onCopy,
  onToggleBlock,
}: SessionMessageItemProps) {
  const { t } = useTranslation();
  const role = group.role.toLowerCase();
  const isTool = group.kind === "tool";
  const isSystem = role === "system";
  const isOuterCollapsible = isTool || isSystem;
  const toolInput = isTool ? getToolInputDisplay(group) : null;
  const hasReasoning = Boolean(group.reasoning.trim());
  const hasContent = Boolean(group.content.trim());
  const hasToolInput = Boolean(group.toolInput?.trim());
  const hasToolOutput = Boolean(group.toolOutput?.trim());
  const toolName =
    group.toolName && group.toolName.toLowerCase() !== "unknown"
      ? group.toolName
      : null;
  const outerBlockKey = isTool
    ? `${group.id}:tool-section`
    : `${group.id}:system-section`;
  const outerDefaultExpanded = isTool
    ? defaultExpandTools
    : defaultExpandSystem;
  const outerExpanded =
    expandedBlockOverrides.get(outerBlockKey) ?? outerDefaultExpanded;
  const reasoningBlockKey = `${group.id}:reasoning-section`;
  const reasoningExpanded =
    expandedBlockOverrides.get(reasoningBlockKey) ?? defaultExpandThinking;

  return (
    <div
      className={cn(
        "rounded-lg border px-3 py-2.5 relative group transition-shadow min-w-0",
        role === "user"
          ? "bg-primary/5 border-primary/20 ml-8"
          : role === "assistant"
            ? "bg-blue-500/5 border-blue-500/20 mr-8"
            : "bg-muted/40 border-border/60",
        isActive && "ring-2 ring-primary ring-offset-2",
      )}
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            className="absolute top-2 right-2 size-6 opacity-0 group-hover:opacity-100 transition-opacity"
            onClick={() => onCopy(getMessageGroupCopyText(group))}
          >
            <Copy className="size-3" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          {t("sessionManager.copyMessage", {
            defaultValue: "复制消息",
          })}
        </TooltipContent>
      </Tooltip>

      {isOuterCollapsible ? (
        <button
          type="button"
          aria-expanded={outerExpanded}
          onClick={() => onToggleBlock(outerBlockKey, !outerExpanded)}
          className="mb-1.5 flex w-full items-center justify-between gap-3 pr-8 text-left text-xs"
        >
          <span
            className={cn("min-w-0 font-semibold", getRoleTone(group.role))}
          >
            {isTool && toolName
              ? `${getRoleLabel(group.role, t)} · ${toolName}`
              : getRoleLabel(group.role, t)}
          </span>
          <span className="flex shrink-0 items-center gap-2 text-muted-foreground">
            {outerExpanded ? (
              <ChevronDown className="size-3.5" />
            ) : (
              <ChevronRight className="size-3.5" />
            )}
            {group.ts && <span>{formatTimestamp(group.ts)}</span>}
          </span>
        </button>
      ) : (
        <div className="mb-1.5 flex items-center justify-between pr-6 text-xs">
          <span className={cn("font-semibold", getRoleTone(group.role))}>
            {getRoleLabel(group.role, t)}
          </span>
          {group.ts && (
            <span className="text-muted-foreground">
              {formatTimestamp(group.ts)}
            </span>
          )}
        </div>
      )}

      {isTool ? (
        outerExpanded && (
          <>
            {hasToolInput && toolInput && (
              <div className="rounded-md bg-muted/60 p-2.5">
                <div className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
                  <TerminalSquare className="size-3.5" />
                  {toolInput.language === "bash"
                    ? t("sessionManager.toolCommand", {
                        defaultValue: "调用命令",
                      })
                    : t("sessionManager.toolArguments", {
                        defaultValue: "调用参数",
                      })}
                </div>
                <MessageTextBlock
                  blockKey={`${group.id}:tool-input`}
                  content={
                    toolInput.language === "bash"
                      ? `$ ${toolInput.text}`
                      : toolInput.text
                  }
                  expandedBlockOverrides={expandedBlockOverrides}
                  onToggleBlock={onToggleBlock}
                  searchQuery={searchQuery}
                  className="font-mono text-xs"
                />
              </div>
            )}
            {hasToolInput && hasToolOutput && <SectionDivider />}
            {hasToolOutput && (
              <MessageTextBlock
                blockKey={`${group.id}:tool-output`}
                content={group.toolOutput ?? ""}
                expandedBlockOverrides={expandedBlockOverrides}
                onToggleBlock={onToggleBlock}
                searchQuery={searchQuery}
                className="font-mono text-xs"
              />
            )}
          </>
        )
      ) : isSystem ? (
        outerExpanded &&
        hasContent && (
          <MessageTextBlock
            blockKey={`${group.id}:system-content`}
            content={group.content}
            expandedBlockOverrides={expandedBlockOverrides}
            onToggleBlock={onToggleBlock}
            searchQuery={searchQuery}
          />
        )
      ) : (
        <>
          {hasReasoning && (
            <div className="rounded-md border border-blue-500/15 bg-blue-500/5 p-2.5">
              <button
                type="button"
                aria-expanded={reasoningExpanded}
                onClick={() =>
                  onToggleBlock(reasoningBlockKey, !reasoningExpanded)
                }
                className="flex w-full items-center justify-between gap-3 text-left text-xs font-medium text-blue-600 transition-colors hover:text-blue-500 dark:text-blue-400 dark:hover:text-blue-300"
              >
                <span className="flex items-center gap-1.5">
                  <Brain className="size-3.5" />
                  {t("sessionManager.thinking", { defaultValue: "Thinking" })}
                </span>
                {reasoningExpanded ? (
                  <ChevronDown className="size-3.5" />
                ) : (
                  <ChevronRight className="size-3.5" />
                )}
              </button>
              {reasoningExpanded && (
                <div className="mt-1.5">
                  <MessageTextBlock
                    blockKey={`${group.id}:reasoning-content`}
                    content={group.reasoning}
                    expandedBlockOverrides={expandedBlockOverrides}
                    onToggleBlock={onToggleBlock}
                    searchQuery={searchQuery}
                    className="text-muted-foreground"
                  />
                </div>
              )}
            </div>
          )}
          {hasReasoning && hasContent && <SectionDivider />}
          {hasContent && (
            <MessageTextBlock
              blockKey={`${group.id}:content`}
              content={group.content}
              expandedBlockOverrides={expandedBlockOverrides}
              onToggleBlock={onToggleBlock}
              searchQuery={searchQuery}
            />
          )}
        </>
      )}
    </div>
  );
});
