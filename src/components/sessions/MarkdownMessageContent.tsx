import {
  Children,
  Fragment,
  cloneElement,
  isValidElement,
  memo,
  useMemo,
  type ReactElement,
  type ReactNode,
} from "react";
import { Copy } from "lucide-react";
import ReactMarkdown, {
  type Components,
  type UrlTransform,
} from "react-markdown";
import remarkBreaks from "remark-breaks";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { highlightText } from "./utils";

const ALLOWED_ELEMENTS = [
  "p",
  "br",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "ul",
  "ol",
  "li",
  "blockquote",
  "strong",
  "em",
  "a",
  "code",
  "pre",
  "hr",
  "img",
];

const safeMarkdownUrl: UrlTransform = (url) => {
  try {
    const parsed = new URL(url);
    return parsed.protocol === "http:" || parsed.protocol === "https:"
      ? parsed.href
      : "";
  } catch {
    return "";
  }
};

const nodeToText = (node: ReactNode): string => {
  if (typeof node === "string" || typeof node === "number") {
    return String(node);
  }
  if (Array.isArray(node)) {
    return node.map(nodeToText).join("");
  }
  if (isValidElement<{ children?: ReactNode }>(node)) {
    return nodeToText(node.props.children);
  }
  return "";
};

const highlightNode = (node: ReactNode, searchQuery?: string): ReactNode => {
  if (!searchQuery) return node;
  if (typeof node === "string") return highlightText(node, searchQuery);
  if (Array.isArray(node)) {
    return node.map((child, index) => (
      <Fragment key={index}>{highlightNode(child, searchQuery)}</Fragment>
    ));
  }
  if (isValidElement<{ children?: ReactNode }>(node)) {
    if (node.type === "mark") return node;
    return cloneElement(
      node,
      {},
      highlightNode(node.props.children, searchQuery),
    );
  }
  return node;
};

interface MarkdownMessageContentProps {
  content: string;
  searchQuery?: string;
  onCopyCode: (content: string) => void;
  onOpenLink: (url: string) => void;
}

interface CodeElementProps {
  className?: string;
  children?: ReactNode;
}

export const MarkdownMessageContent = memo(function MarkdownMessageContent({
  content,
  searchQuery,
  onCopyCode,
  onOpenLink,
}: MarkdownMessageContentProps) {
  const { t } = useTranslation();

  const components = useMemo<Components>(
    () => ({
      p: ({ children }) => (
        <p className="my-2 first:mt-0 last:mb-0">
          {highlightNode(children, searchQuery)}
        </p>
      ),
      h1: ({ children }) => (
        <h1 className="mb-2 mt-4 text-base font-semibold first:mt-0">
          {highlightNode(children, searchQuery)}
        </h1>
      ),
      h2: ({ children }) => (
        <h2 className="mb-2 mt-4 text-base font-semibold first:mt-0">
          {highlightNode(children, searchQuery)}
        </h2>
      ),
      h3: ({ children }) => (
        <h3 className="mb-1.5 mt-3 text-sm font-semibold first:mt-0">
          {highlightNode(children, searchQuery)}
        </h3>
      ),
      h4: ({ children }) => (
        <h4 className="mb-1.5 mt-3 text-sm font-semibold first:mt-0">
          {highlightNode(children, searchQuery)}
        </h4>
      ),
      h5: ({ children }) => (
        <h5 className="mb-1.5 mt-3 text-sm font-semibold first:mt-0">
          {highlightNode(children, searchQuery)}
        </h5>
      ),
      h6: ({ children }) => (
        <h6 className="mb-1.5 mt-3 text-sm font-semibold first:mt-0">
          {highlightNode(children, searchQuery)}
        </h6>
      ),
      ul: ({ children }) => (
        <ul className="my-2 list-disc space-y-1 pl-5 first:mt-0 last:mb-0">
          {children}
        </ul>
      ),
      ol: ({ children }) => (
        <ol className="my-2 list-decimal space-y-1 pl-5 first:mt-0 last:mb-0">
          {children}
        </ol>
      ),
      li: ({ children }) => (
        <li className="pl-0.5">{highlightNode(children, searchQuery)}</li>
      ),
      blockquote: ({ children }) => (
        <blockquote className="my-3 border-l-2 border-primary/35 pl-3 text-muted-foreground">
          {children}
        </blockquote>
      ),
      a: ({ href, children }) => {
        const url = href?.trim();
        if (!url) {
          return <span>{highlightNode(children, searchQuery)}</span>;
        }
        return (
          <a
            href={url}
            className="font-medium text-primary underline decoration-primary/50 underline-offset-2 transition-colors hover:text-primary/80"
            onClick={(event) => {
              event.preventDefault();
              onOpenLink(url);
            }}
          >
            {highlightNode(children, searchQuery)}
          </a>
        );
      },
      code: ({ children, className }) =>
        className?.includes("language-") ? (
          <code className="font-mono">{children}</code>
        ) : (
          <code className="rounded bg-muted px-1 py-0.5 font-mono text-[0.85em]">
            {highlightNode(children, searchQuery)}
          </code>
        ),
      pre: ({ children }) => {
        const codeElement = Children.toArray(children).find((child) =>
          isValidElement<CodeElementProps>(child),
        ) as ReactElement<CodeElementProps> | undefined;
        const className = codeElement?.props.className ?? "";
        const language = className.match(/language-([^\s]+)/)?.[1];
        const code = nodeToText(codeElement?.props.children).replace(/\n$/, "");

        return (
          <div className="my-3 overflow-hidden rounded-lg border border-border/80 bg-muted/55 first:mt-0 last:mb-0">
            <div className="flex items-center justify-between gap-3 border-b border-border/70 bg-muted/70 px-3 py-1.5">
              <span className="truncate font-mono text-[11px] text-muted-foreground">
                {language ||
                  t("sessionManager.codeBlock", { defaultValue: "Code" })}
              </span>
              <Button
                type="button"
                variant="ghost"
                size="icon"
                className="size-6 shrink-0"
                aria-label={t("sessionManager.copyCode", {
                  defaultValue: "复制代码",
                })}
                title={t("sessionManager.copyCode", {
                  defaultValue: "复制代码",
                })}
                onClick={() => onCopyCode(code)}
              >
                <Copy className="size-3" />
              </Button>
            </div>
            <pre className="max-w-full overflow-x-auto p-3 font-mono text-xs leading-5">
              <code>
                {searchQuery ? highlightText(code, searchQuery) : code}
              </code>
            </pre>
          </div>
        );
      },
      hr: () => <hr className="my-4 border-border/80" />,
      img: ({ alt }) =>
        alt ? (
          <span className="my-2 inline-block rounded border border-border/70 bg-muted/50 px-2 py-1 text-xs text-muted-foreground">
            {t("sessionManager.imagePlaceholder", {
              alt,
              defaultValue: "[Image: {{alt}}]",
            })}
          </span>
        ) : null,
    }),
    [onCopyCode, onOpenLink, searchQuery, t],
  );

  return (
    <div className="min-w-0 break-words [overflow-wrap:anywhere]">
      <ReactMarkdown
        allowedElements={ALLOWED_ELEMENTS}
        components={components}
        remarkPlugins={[remarkBreaks]}
        skipHtml
        urlTransform={safeMarkdownUrl}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
});
