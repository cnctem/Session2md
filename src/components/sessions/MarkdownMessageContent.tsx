import {
  Children,
  Fragment,
  cloneElement,
  isValidElement,
  memo,
  useCallback,
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
import remarkGfm from "remark-gfm";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { localFilesApi } from "@/lib/api/localFiles";
import { showLocalFileErrorToast } from "@/lib/markdown/localFileErrors";
import {
  classifyMarkdownLink,
  decodeLocalPath,
} from "@/lib/markdown/localPaths";
import { MarkdownImage } from "./MarkdownImage";
import { highlightTerms } from "./utils";

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
  "table",
  "thead",
  "tbody",
  "tfoot",
  "tr",
  "th",
  "td",
  "strong",
  "em",
  "a",
  "code",
  "pre",
  "hr",
  "img",
];

const safeMarkdownUrl: UrlTransform = (url) => {
  const target = classifyMarkdownLink(url);
  if (target.kind === "external") return target.url;
  if (target.kind === "local") return url;
  return "";
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

const highlightNode = (node: ReactNode, searchTerms?: string[]): ReactNode => {
  if (!searchTerms?.length) return node;
  if (typeof node === "string") return highlightTerms(node, searchTerms);
  if (Array.isArray(node)) {
    return node.map((child, index) => (
      <Fragment key={index}>{highlightNode(child, searchTerms)}</Fragment>
    ));
  }
  if (isValidElement<{ children?: ReactNode }>(node)) {
    if (node.type === "mark") return node;
    return cloneElement(
      node,
      {},
      highlightNode(node.props.children, searchTerms),
    );
  }
  return node;
};

interface MarkdownMessageContentProps {
  content: string;
  searchTerms?: string[];
  /** Session working directory used to resolve relative links and images. */
  baseDir?: string | null;
  onCopyCode: (content: string) => void;
  onOpenLink: (url: string) => void;
}

interface CodeElementProps {
  className?: string;
  children?: ReactNode;
}

export const MarkdownMessageContent = memo(function MarkdownMessageContent({
  content,
  searchTerms,
  baseDir,
  onCopyCode,
  onOpenLink,
}: MarkdownMessageContentProps) {
  const { t } = useTranslation();

  const handleOpenLocalPath = useCallback(
    (path: string) => {
      void localFilesApi
        .openPath(path, baseDir)
        .catch((error: unknown) => showLocalFileErrorToast(error, t));
    },
    [baseDir, t],
  );

  const components = useMemo<Components>(
    () => ({
      p: ({ children }) => (
        <p className="my-2 first:mt-0 last:mb-0">
          {highlightNode(children, searchTerms)}
        </p>
      ),
      h1: ({ children }) => (
        <h1 className="mb-2 mt-4 text-base font-semibold first:mt-0">
          {highlightNode(children, searchTerms)}
        </h1>
      ),
      h2: ({ children }) => (
        <h2 className="mb-2 mt-4 text-base font-semibold first:mt-0">
          {highlightNode(children, searchTerms)}
        </h2>
      ),
      h3: ({ children }) => (
        <h3 className="mb-1.5 mt-3 text-sm font-semibold first:mt-0">
          {highlightNode(children, searchTerms)}
        </h3>
      ),
      h4: ({ children }) => (
        <h4 className="mb-1.5 mt-3 text-sm font-semibold first:mt-0">
          {highlightNode(children, searchTerms)}
        </h4>
      ),
      h5: ({ children }) => (
        <h5 className="mb-1.5 mt-3 text-sm font-semibold first:mt-0">
          {highlightNode(children, searchTerms)}
        </h5>
      ),
      h6: ({ children }) => (
        <h6 className="mb-1.5 mt-3 text-sm font-semibold first:mt-0">
          {highlightNode(children, searchTerms)}
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
        <li className="pl-0.5">{highlightNode(children, searchTerms)}</li>
      ),
      blockquote: ({ children }) => (
        <blockquote className="my-3 border-l-2 border-primary/35 pl-3 text-muted-foreground">
          {children}
        </blockquote>
      ),
      table: ({ children }) => (
        <div className="my-3 max-w-full overflow-x-auto rounded-md border border-border/80 first:mt-0 last:mb-0">
          <table className="w-full min-w-max border-collapse text-left text-xs leading-5">
            {children}
          </table>
        </div>
      ),
      thead: ({ children }) => (
        <thead className="bg-muted/60">{children}</thead>
      ),
      tbody: ({ children }) => <tbody>{children}</tbody>,
      tr: ({ children }) => (
        <tr className="border-b border-border/70 last:border-b-0">
          {children}
        </tr>
      ),
      th: ({ children, style }) => (
        <th
          className="border-r border-border/60 px-3 py-2 align-top font-medium last:border-r-0 [&_ol]:my-0 [&_p]:my-0 [&_ul]:my-0"
          style={style}
        >
          {highlightNode(children, searchTerms)}
        </th>
      ),
      td: ({ children, style }) => (
        <td
          className="border-r border-border/60 px-3 py-2 align-top last:border-r-0 [&_ol]:my-0 [&_p]:my-0 [&_ul]:my-0"
          style={style}
        >
          {highlightNode(children, searchTerms)}
        </td>
      ),
      a: ({ href, children }) => {
        const url = href?.trim();
        const target = url ? classifyMarkdownLink(url) : null;
        if (!url || !target || target.kind === "unsupported") {
          return <span>{highlightNode(children, searchTerms)}</span>;
        }
        return (
          <a
            href={url}
            title={
              target.kind === "local" ? decodeLocalPath(target.path) : undefined
            }
            className="font-medium text-primary underline decoration-primary/50 underline-offset-2 transition-colors hover:text-primary/80"
            onClick={(event) => {
              event.preventDefault();
              if (target.kind === "external") {
                onOpenLink(target.url);
              } else {
                handleOpenLocalPath(decodeLocalPath(target.path));
              }
            }}
          >
            {highlightNode(children, searchTerms)}
          </a>
        );
      },
      code: ({ children, className }) =>
        className?.includes("language-") ? (
          <code className="font-mono">{children}</code>
        ) : (
          <code className="rounded bg-muted px-1 py-0.5 font-mono text-[0.85em]">
            {highlightNode(children, searchTerms)}
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
                {searchTerms?.length ? highlightTerms(code, searchTerms) : code}
              </code>
            </pre>
          </div>
        );
      },
      hr: () => <hr className="my-4 border-border/80" />,
      img: ({ src, alt }) => (
        <MarkdownImage
          key={src}
          src={src}
          alt={alt}
          baseDir={baseDir}
          onOpenLink={onOpenLink}
        />
      ),
    }),
    [baseDir, handleOpenLocalPath, onCopyCode, onOpenLink, searchTerms, t],
  );

  return (
    <div className="min-w-0 break-words [overflow-wrap:anywhere]">
      <ReactMarkdown
        allowedElements={ALLOWED_ELEMENTS}
        components={components}
        remarkPlugins={[remarkGfm, remarkBreaks]}
        skipHtml
        urlTransform={safeMarkdownUrl}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
});
