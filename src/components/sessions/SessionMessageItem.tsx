import { memo, useState } from "react";
import {
  Brain,
  ChevronDown,
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
  content: string;
  searchQuery?: string;
  className?: string;
}

function MessageTextBlock({
  content,
  searchQuery,
  className,
}: MessageTextBlockProps) {
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
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
          onClick={() => setExpanded((value) => !value)}
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
  searchQuery?: string;
  onCopy: (content: string) => void;
}

export const SessionMessageItem = memo(function SessionMessageItem({
  group,
  isActive,
  searchQuery,
  onCopy,
}: SessionMessageItemProps) {
  const { t } = useTranslation();
  const role = group.role.toLowerCase();
  const isTool = group.kind === "tool";
  const toolInput = isTool ? getToolInputDisplay(group) : null;
  const hasReasoning = Boolean(group.reasoning.trim());
  const hasContent = Boolean(group.content.trim());
  const hasToolInput = Boolean(group.toolInput?.trim());
  const hasToolOutput = Boolean(group.toolOutput?.trim());
  const toolName =
    group.toolName && group.toolName.toLowerCase() !== "unknown"
      ? group.toolName
      : null;

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

      <div className="flex items-center justify-between text-xs mb-1.5 pr-6">
        <span className={cn("font-semibold", getRoleTone(group.role))}>
          {isTool && toolName
            ? `${getRoleLabel(group.role, t)} · ${toolName}`
            : getRoleLabel(group.role, t)}
        </span>
        {group.ts && (
          <span className="text-muted-foreground">
            {formatTimestamp(group.ts)}
          </span>
        )}
      </div>

      {isTool ? (
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
                content={
                  toolInput.language === "bash"
                    ? `$ ${toolInput.text}`
                    : toolInput.text
                }
                searchQuery={searchQuery}
                className="font-mono text-xs"
              />
            </div>
          )}
          {hasToolInput && hasToolOutput && <SectionDivider />}
          {hasToolOutput && (
            <MessageTextBlock
              content={group.toolOutput ?? ""}
              searchQuery={searchQuery}
              className="font-mono text-xs"
            />
          )}
        </>
      ) : (
        <>
          {hasReasoning && (
            <div className="rounded-md border border-blue-500/15 bg-blue-500/5 p-2.5">
              <div className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-blue-600 dark:text-blue-400">
                <Brain className="size-3.5" />
                {t("sessionManager.thinking", { defaultValue: "Thinking" })}
              </div>
              <MessageTextBlock
                content={group.reasoning}
                searchQuery={searchQuery}
                className="text-muted-foreground"
              />
            </div>
          )}
          {hasReasoning && hasContent && <SectionDivider />}
          {hasContent && (
            <MessageTextBlock
              content={group.content}
              searchQuery={searchQuery}
            />
          )}
        </>
      )}
    </div>
  );
});
