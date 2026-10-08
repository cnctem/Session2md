import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SessionManagerPage } from "@/components/sessions/SessionManagerPage";
import { sessionsApi } from "@/lib/api/sessions";
import type { SessionMessage, SessionMeta } from "@/types";
import {
  setHiddenSessionProviders,
  setSession2mdExportOptions,
  setSession2mdPromptBeforeExport,
  setSession2mdDefaultExpansion,
  setSession2mdRenderMarkdown,
  setSessionFixtures,
} from "../msw/state";

const toastSuccessMock = vi.fn();
const toastErrorMock = vi.fn();
const GROUP_EXPANSION_STORAGE_KEY =
  "session2md.sessionManager.groupExpansionState";

vi.mock("sonner", () => ({
  toast: {
    success: (...args: unknown[]) => toastSuccessMock(...args),
    error: (...args: unknown[]) => toastErrorMock(...args),
  },
}));

vi.mock("@/components/sessions/SessionToc", () => ({
  SessionTocSidebar: () => null,
  SessionTocDialog: () => null,
}));

const renderPage = (props: { minimalMode?: boolean } = {}) => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <SessionManagerPage {...props} />
    </QueryClientProvider>,
  );
};

const switchToGroupedView = async (
  user: ReturnType<typeof userEvent.setup>,
) => {
  await user.click(
    screen.getByRole("combobox", { name: "sessionManager.viewModeTooltip" }),
  );
  await user.click(
    await screen.findByRole("option", {
      name: "sessionManager.viewModeGrouped",
    }),
  );
};

describe("SessionManagerPage", () => {
  beforeEach(() => {
    toastSuccessMock.mockReset();
    toastErrorMock.mockReset();
    window.localStorage.clear();

    const sessions: SessionMeta[] = [
      {
        providerId: "codex",
        sessionId: "codex-session-1",
        title: "Alpha Session",
        projectDir: "/mock/codex",
        lastActiveAt: 20,
        sourcePath: "/mock/codex/session-1.jsonl",
        resumeCommand: "codex resume codex-session-1",
      },
      {
        providerId: "codex",
        sessionId: "codex-session-2",
        title: "Codex Docs Session",
        projectDir: "/mock/docs",
        lastActiveAt: 15,
        sourcePath: "/mock/docs/session-2.jsonl",
      },
      {
        providerId: "claude",
        sessionId: "claude-session-1",
        title: "Claude Session",
        projectDir: "/mock/claude",
        lastActiveAt: 30,
        sourcePath: "/mock/claude/session-1.jsonl",
      },
      {
        providerId: "pi",
        sessionId: "pi-session-1",
        title: "Pi Session",
        projectDir: "/mock/pi",
        lastActiveAt: 40,
        sourcePath: "/mock/pi/session-1.jsonl",
      },
    ];
    const messages: Record<string, SessionMessage[]> = {
      "codex:/mock/codex/session-1.jsonl": [
        { role: "user", content: "alpha", ts: 20 },
      ],
      "codex:/mock/docs/session-2.jsonl": [
        { role: "user", content: "codex docs", ts: 15 },
      ],
      "claude:/mock/claude/session-1.jsonl": [
        { role: "assistant", content: "claude", ts: 30 },
      ],
      "pi:/mock/pi/session-1.jsonl": [
        { role: "user", content: "pi prompt", ts: 40 },
        { role: "assistant", content: "pi answer", ts: 41 },
      ],
    };
    setSessionFixtures(sessions, messages);
  });

  it("starts on the all-provider session view", async () => {
    renderPage();

    expect(
      await screen.findByRole("combobox", {
        name: "sessionManager.providerFilterTooltip",
      }),
    ).toHaveTextContent("sessionManager.providerFilterAll");
    expect(await screen.findByText("Alpha Session")).toBeInTheDocument();
    expect(screen.getAllByText("Claude Session")).not.toHaveLength(0);
    expect(
      screen.getByRole("button", { name: "sessionManager.advanced.search" }),
    ).toBeVisible();
  });

  it("does not submit an empty search with only default filters", async () => {
    const user = userEvent.setup();
    const searchSpy = vi.spyOn(sessionsApi, "searchAdvanced");
    renderPage();

    await user.click(
      await screen.findByRole("button", {
        name: "sessionManager.advanced.search",
      }),
    );

    expect(
      screen.getByText("sessionManager.advanced.noConditions"),
    ).toBeVisible();
    expect(searchSpy).not.toHaveBeenCalled();
    searchSpy.mockRestore();
  });

  it("clears the search from inside the input", async () => {
    const user = userEvent.setup();
    renderPage();

    const input = await screen.findByPlaceholderText(
      "sessionManager.searchPlaceholder",
    );
    await user.type(input, "alpha");
    await user.click(screen.getByRole("button", { name: "common.clear" }));

    expect(input).toHaveValue("");
  });

  it("submits advanced search with Enter while options are closed", async () => {
    const user = userEvent.setup();
    const searchSpy = vi.spyOn(sessionsApi, "searchAdvanced");
    renderPage();

    await user.type(
      await screen.findByPlaceholderText("sessionManager.searchPlaceholder"),
      "alpha",
    );
    await user.keyboard("{Enter}");

    await waitFor(() => expect(searchSpy).toHaveBeenCalledTimes(1));
    searchSpy.mockRestore();
  });

  it("opens advanced options when a hidden validation error is submitted", async () => {
    const user = userEvent.setup();
    renderPage();

    await user.click(
      await screen.findByRole("button", {
        name: "sessionManager.advanced.open",
      }),
    );
    await user.clear(
      screen.getByLabelText("sessionManager.advanced.activeFrom"),
    );
    await user.clear(screen.getByLabelText("sessionManager.advanced.activeTo"));
    await user.click(
      screen.getByRole("button", { name: "sessionManager.advanced.open" }),
    );
    await user.click(
      screen.getByRole("button", { name: "sessionManager.advanced.search" }),
    );

    expect(
      screen.getByText("sessionManager.advanced.noConditions"),
    ).toBeVisible();
  });

  it("validates and runs an advanced full-text search", async () => {
    const user = userEvent.setup();
    renderPage();

    await user.click(
      await screen.findByRole("button", {
        name: "sessionManager.advanced.open",
      }),
    );

    const formatDate = (date: Date) =>
      `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
    const today = new Date();
    const sevenDaysAgo = new Date(today);
    sevenDaysAgo.setDate(today.getDate() - 7);
    const activeFrom = screen.getByLabelText(
      "sessionManager.advanced.activeFrom",
    );
    const activeTo = screen.getByLabelText("sessionManager.advanced.activeTo");
    expect(activeFrom).toHaveValue(formatDate(sevenDaysAgo));
    expect(activeTo).toHaveValue(formatDate(today));

    await user.clear(activeFrom);
    await user.clear(activeTo);
    await user.click(
      screen.getByRole("button", {
        name: "sessionManager.advanced.search",
      }),
    );
    expect(
      screen.getByText("sessionManager.advanced.noConditions"),
    ).toBeInTheDocument();

    await user.type(
      screen.getByPlaceholderText("sessionManager.searchPlaceholder"),
      "claude",
    );
    await user.click(
      screen.getByRole("button", {
        name: "sessionManager.advanced.search",
      }),
    );

    expect(
      await screen.findByText("sessionManager.advanced.resultCount"),
    ).toBeInTheDocument();
    expect(await screen.findAllByText("Claude Session")).not.toHaveLength(0);
    expect(await screen.findAllByText("claude")).not.toHaveLength(0);
  });

  it("exports the selected session as Markdown", async () => {
    const exportSpy = vi
      .spyOn(sessionsApi, "exportMarkdown")
      .mockResolvedValueOnce("/tmp/Alpha Session.md");
    renderPage();

    fireEvent.click(
      await screen.findByRole("button", { name: /Alpha Session/i }),
    );
    const exportButton = await screen.findByRole("button", {
      name: "sessionManager.export",
    });
    await waitFor(() => expect(exportButton).not.toBeDisabled());
    fireEvent.click(exportButton);
    fireEvent.click(
      await screen.findByRole("button", {
        name: "sessionManager.exportOptions.confirm",
      }),
    );

    await waitFor(() =>
      expect(exportSpy).toHaveBeenCalledWith(
        "Alpha Session.md",
        "## User\n\nalpha\n",
      ),
    );
    expect(toastSuccessMock).toHaveBeenCalled();
  });

  it("exports directly when prompt before export is disabled", async () => {
    setSession2mdPromptBeforeExport(false);
    const exportSpy = vi
      .spyOn(sessionsApi, "exportMarkdown")
      .mockResolvedValueOnce("/tmp/Alpha Session.md");
    renderPage();

    fireEvent.click(
      await screen.findByRole("button", { name: /Alpha Session/i }),
    );
    const exportButton = await screen.findByRole("button", {
      name: "sessionManager.export",
    });
    await waitFor(() => expect(exportButton).not.toBeDisabled());
    fireEvent.click(exportButton);

    await waitFor(() =>
      expect(exportSpy).toHaveBeenCalledWith(
        "Alpha Session.md",
        "## User\n\nalpha\n",
      ),
    );
    expect(
      screen.queryByText("sessionManager.exportOptions.title"),
    ).not.toBeInTheDocument();
  });

  it("uses one-off choices from the export prompt", async () => {
    setSession2mdExportOptions({ exportThinking: true });
    setSessionFixtures(
      [
        {
          providerId: "codex",
          sessionId: "thinking-session",
          title: "Thinking Session",
          sourcePath: "/mock/codex/thinking-session.jsonl",
        },
      ],
      {
        "codex:/mock/codex/thinking-session.jsonl": [
          { role: "user", content: "question" },
          {
            role: "assistant",
            content: "private reasoning",
            kind: "reasoning",
          },
          { role: "assistant", content: "answer", kind: "text" },
        ],
      },
    );
    const exportSpy = vi
      .spyOn(sessionsApi, "exportMarkdown")
      .mockResolvedValueOnce("/tmp/Thinking Session.md");
    renderPage();

    fireEvent.click(
      await screen.findByRole("button", { name: /Thinking Session/i }),
    );
    const exportButton = await screen.findByRole("button", {
      name: "sessionManager.export",
    });
    await waitFor(() => expect(exportButton).not.toBeDisabled());
    fireEvent.click(exportButton);

    const checkboxes = await screen.findAllByRole("checkbox");
    expect(checkboxes[0]).toBeChecked();
    fireEvent.click(checkboxes[0]);
    fireEvent.click(
      screen.getByRole("button", {
        name: "sessionManager.exportOptions.confirm",
      }),
    );

    await waitFor(() =>
      expect(exportSpy).toHaveBeenCalledWith(
        "Thinking Session.md",
        "## User\n\nquestion\n\n## Assistant\n\nanswer\n",
      ),
    );
  });

  it("shows the CC Switch-aligned source and resume command rows", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    renderPage();

    fireEvent.click(
      await screen.findByRole("button", { name: /Alpha Session/i }),
    );
    const copyDirectoryButton = await screen.findByRole("button", {
      name: "sessionManager.copyProjectDir",
    });
    const copySourcePathButton = screen.getByRole("button", {
      name: "sessionManager.copySourcePath",
    });
    const copyCommandButton = screen.getByRole("button", {
      name: "sessionManager.copyCommand",
    });

    expect(copyDirectoryButton).toHaveTextContent("codex");
    expect(copySourcePathButton).toHaveTextContent("session-1.jsonl");
    expect(screen.getByText("codex resume codex-session-1")).toBeVisible();

    fireEvent.click(copyDirectoryButton);
    fireEvent.click(copySourcePathButton);
    fireEvent.click(copyCommandButton);

    await waitFor(() =>
      expect(writeText).toHaveBeenNthCalledWith(1, "/mock/codex"),
    );
    expect(writeText).toHaveBeenNthCalledWith(2, "/mock/codex/session-1.jsonl");
    expect(writeText).toHaveBeenNthCalledWith(
      3,
      "codex resume codex-session-1",
    );
  });

  it("omits the resume command row when a session has no command", async () => {
    renderPage();

    expect(await screen.findByText("Pi Session")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "sessionManager.copyCommand" }),
    ).not.toBeInTheDocument();
  });

  it("exports filtered Codex messages as Markdown", async () => {
    const exportSpy = vi
      .spyOn(sessionsApi, "exportMarkdown")
      .mockResolvedValueOnce("/tmp/Filtered Session.md");

    setSessionFixtures(
      [
        {
          providerId: "codex",
          sessionId: "filtered-session",
          title: "Filtered Session",
          sourcePath: "/mock/codex/filtered-session.jsonl",
        },
      ],
      {
        "codex:/mock/codex/filtered-session.jsonl": [
          {
            role: "user",
            content: "# AGENTS.md instructions for /mock/codex",
          },
          {
            role: "user",
            content:
              "<environment_context>\n<cwd>/mock/codex</cwd>\n</environment_context>",
          },
          { role: "user", content: "Keep this request" },
          { role: "tool", content: "[Tool: shell]\n[Tool: shell]" },
          { role: "assistant", content: "Here is the answer." },
        ],
      },
    );

    renderPage();

    const exportButton = await screen.findByRole("button", {
      name: "sessionManager.export",
    });
    await waitFor(() => expect(exportButton).not.toBeDisabled());
    fireEvent.click(exportButton);
    fireEvent.click(
      await screen.findByRole("button", {
        name: "sessionManager.exportOptions.confirm",
      }),
    );

    await waitFor(() =>
      expect(exportSpy).toHaveBeenCalledWith(
        "Filtered Session.md",
        "## User\n\nKeep this request\n\n" +
          "## Assistant\n\nHere is the answer.\n",
      ),
    );
    expect(toastSuccessMock).toHaveBeenCalled();

    exportSpy.mockRestore();
  });

  it("disables export when only assistant tool messages remain", async () => {
    setSessionFixtures(
      [
        {
          providerId: "codex",
          sessionId: "tool-only-session",
          title: "Tool-only Session",
          sourcePath: "/mock/codex/tool-only-session.jsonl",
        },
      ],
      {
        "codex:/mock/codex/tool-only-session.jsonl": [
          { role: "assistant", content: "[Tool: bash]\n[Tool: bash]" },
        ],
      },
    );

    renderPage();

    const exportButton = await screen.findByRole("button", {
      name: "sessionManager.export",
    });
    await waitFor(() => expect(exportButton).toBeDisabled());
  });

  it("collapses thinking, tools, and system messages until opened", async () => {
    const user = userEvent.setup();
    setSessionFixtures(
      [
        {
          providerId: "dsh",
          sessionId: "dsh-detailed-session",
          title: "Detailed Session",
          sourcePath: "/mock/dsh/detailed-session.jsonl",
        },
      ],
      {
        "dsh:/mock/dsh/detailed-session.jsonl": [
          {
            role: "system",
            content: "hidden system instructions\nsystem details",
            kind: "text",
          },
          { role: "user", content: "run it", kind: "text" },
          { role: "assistant", content: "hidden thinking", kind: "reasoning" },
          { role: "assistant", content: "visible answer", kind: "text" },
          {
            role: "tool",
            content: '{"command":"ls -la"}',
            kind: "toolCall",
            toolCallId: "call-1",
            toolName: "bash",
          },
          {
            role: "tool",
            content: "file-a\nfile-b",
            kind: "toolResult",
            toolCallId: "call-1",
            toolName: "bash",
          },
        ],
      },
    );

    renderPage();

    expect(await screen.findByText("visible answer")).toBeInTheDocument();
    expect(screen.queryByText("hidden thinking")).not.toBeInTheDocument();
    expect(screen.queryByText("$ ls -la")).not.toBeInTheDocument();
    expect(screen.queryByText(/file-a\s+file-b/)).not.toBeInTheDocument();
    expect(screen.queryByText(/system details/)).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Thinking" }));
    expect(await screen.findByText("hidden thinking")).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", {
        name: /sessionManager\.roleTool · bash/,
      }),
    );
    const command = await screen.findByText("$ ls -la");
    const toolBubble = command.closest(".rounded-lg.border");
    expect(toolBubble).not.toHaveClass("bg-purple-500/5");
    expect(toolBubble).toHaveClass("bg-muted/40");
    expect(screen.getByText(/file-a\s+file-b/)).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", {
        name: "sessionManager.roleSystem · hidden system instructions",
      }),
    );
    expect(await screen.findByText(/system details/)).toBeInTheDocument();
  });

  it("shows only user and assistant text in minimal mode", async () => {
    setSessionFixtures(
      [
        {
          providerId: "codex",
          sessionId: "minimal-session",
          title: "Minimal Session",
          sourcePath: "/mock/codex/minimal-session.jsonl",
        },
      ],
      {
        "codex:/mock/codex/minimal-session.jsonl": [
          {
            role: "user",
            content:
              "# AGENTS.md instructions for /tmp/project\n<INSTRUCTIONS>context</INSTRUCTIONS>",
            kind: "text",
          },
          {
            role: "user",
            content:
              "<environment_context>\n<cwd>/tmp/project</cwd>\n</environment_context>",
            kind: "text",
          },
          {
            role: "system",
            content: "hidden system instructions",
            kind: "text",
          },
          { role: "user", content: "visible question", kind: "text" },
          { role: "assistant", content: "hidden thinking", kind: "reasoning" },
          { role: "assistant", content: "visible answer", kind: "text" },
          {
            role: "tool",
            content: '{"command":"ls -la"}',
            kind: "toolCall",
            toolCallId: "minimal-call-1",
            toolName: "bash",
          },
          {
            role: "tool",
            content: "file-a\nfile-b",
            kind: "toolResult",
            toolCallId: "minimal-call-1",
            toolName: "bash",
          },
        ],
      },
    );

    renderPage({ minimalMode: true });

    expect(await screen.findByText("visible question")).toBeInTheDocument();
    expect(await screen.findByText("visible answer")).toBeInTheDocument();
    expect(
      screen.queryByText("hidden system instructions"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText(/AGENTS\.md instructions for/),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/environment_context/)).not.toBeInTheDocument();
    expect(screen.queryByText("hidden thinking")).not.toBeInTheDocument();
    expect(screen.queryByText("$ ls -la")).not.toBeInTheDocument();
    expect(screen.queryByText(/file-a\s+file-b/)).not.toBeInTheDocument();
  });

  it("renders non-user messages as system and expands each one on demand", async () => {
    const user = userEvent.setup();
    setSessionFixtures(
      [
        {
          providerId: "codex",
          sessionId: "collapsed-context-session",
          title: "Collapsed Context Session",
          sourcePath: "/mock/codex/collapsed-context-session.jsonl",
        },
      ],
      {
        "codex:/mock/codex/collapsed-context-session.jsonl": [
          {
            role: "user",
            content:
              "# AGENTS.md instructions for /mock/codex\n<INSTRUCTIONS>agents body</INSTRUCTIONS>",
          },
          {
            role: "user",
            content:
              "<environment_context>\n<cwd>/mock/codex</cwd>\n</environment_context>",
          },
          { role: "user", content: "Keep this request" },
        ],
      },
    );

    renderPage();

    expect(await screen.findByText("Keep this request")).toBeInTheDocument();
    expect(
      screen.queryByText(/AGENTS\.md instructions for/),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/environment_context/)).not.toBeInTheDocument();
    expect(
      screen.getAllByRole("button", {
        name: "sessionManager.roleSystem · 2",
      }),
    ).toHaveLength(1);

    await user.click(
      screen.getByRole("button", {
        name: "sessionManager.roleSystem · 2",
      }),
    );

    const agentsItem = screen.getByRole("button", {
      name: "sessionManager.roleSystem · # AGENTS.md instructions for /mock/codex",
    });
    const environmentItem = screen.getByRole("button", {
      name: "sessionManager.roleSystem · <environment_context>",
    });
    expect(screen.queryByText(/agents body/)).not.toBeInTheDocument();
    expect(
      screen.queryByText(/<cwd>\/mock\/codex<\/cwd>/),
    ).not.toBeInTheDocument();

    await user.click(agentsItem);

    expect(await screen.findByText(/agents body/)).toBeInTheDocument();
    expect(
      screen.queryByText(/<cwd>\/mock\/codex<\/cwd>/),
    ).not.toBeInTheDocument();

    await user.click(environmentItem);

    expect(screen.getByText(/<cwd>\/mock\/codex<\/cwd>/)).toBeInTheDocument();
  });

  it("uses the existing system expansion setting for non-user messages", async () => {
    setSession2mdDefaultExpansion({ defaultExpandSystem: true });
    setSessionFixtures(
      [
        {
          providerId: "codex",
          sessionId: "uncollapsed-context-session",
          title: "Uncollapsed Context Session",
          sourcePath: "/mock/codex/uncollapsed-context-session.jsonl",
        },
      ],
      {
        "codex:/mock/codex/uncollapsed-context-session.jsonl": [
          {
            role: "user",
            content: "# AGENTS.md instructions for /mock/codex",
          },
          {
            role: "user",
            content:
              "<environment_context>\n<cwd>/mock/codex</cwd>\n</environment_context>",
          },
          { role: "user", content: "Keep this request" },
        ],
      },
    );

    renderPage();

    expect(
      await screen.findByRole("button", {
        name: "sessionManager.roleSystem · 2",
      }),
    ).toHaveAttribute("aria-expanded", "true");
    expect(
      screen.getByRole("button", {
        name: "sessionManager.roleSystem · # AGENTS.md instructions for /mock/codex",
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: "sessionManager.roleSystem · <environment_context>",
      }),
    ).toBeInTheDocument();
    expect(screen.queryByText(/agents body/)).not.toBeInTheDocument();
  });

  it("renders user and assistant message bodies as Markdown by default", async () => {
    setSessionFixtures(
      [
        {
          providerId: "codex",
          sessionId: "markdown-session",
          title: "Markdown Session",
          sourcePath: "/mock/codex/markdown-session.jsonl",
        },
      ],
      {
        "codex:/mock/codex/markdown-session.jsonl": [
          { role: "user", content: "# User heading\n\n- one", kind: "text" },
          { role: "assistant", content: "**AI answer**", kind: "text" },
        ],
      },
    );

    renderPage();

    expect(
      await screen.findByRole("heading", { level: 1, name: "User heading" }),
    ).toBeInTheDocument();
    expect(screen.getByText("AI answer").tagName).toBe("STRONG");
  });

  it("shows raw Markdown when message rendering is disabled", async () => {
    setSession2mdRenderMarkdown(false);
    setSessionFixtures(
      [
        {
          providerId: "codex",
          sessionId: "raw-markdown-session",
          title: "Raw Markdown Session",
          sourcePath: "/mock/codex/raw-markdown-session.jsonl",
        },
      ],
      {
        "codex:/mock/codex/raw-markdown-session.jsonl": [
          { role: "user", content: "# Raw heading", kind: "text" },
        ],
      },
    );

    renderPage();

    expect(await screen.findByText("# Raw heading")).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { level: 1, name: "Raw heading" }),
    ).not.toBeInTheDocument();
  });

  it("opens Markdown links and copies fenced code independently", async () => {
    const openExternal = vi
      .spyOn(sessionsApi, "openExternalUrl")
      .mockResolvedValueOnce(true);
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    setSessionFixtures(
      [
        {
          providerId: "codex",
          sessionId: "rich-markdown-session",
          title: "Rich Markdown Session",
          sourcePath: "/mock/codex/rich-markdown-session.jsonl",
        },
      ],
      {
        "codex:/mock/codex/rich-markdown-session.jsonl": [
          {
            role: "assistant",
            content: [
              "[Open docs](https://example.com/docs)",
              "",
              "```ts",
              "const value = 1;",
              "```",
            ].join("\n"),
            kind: "text",
          },
        ],
      },
    );

    renderPage();

    fireEvent.click(await screen.findByRole("link", { name: "Open docs" }));
    expect(openExternal).toHaveBeenCalledWith("https://example.com/docs");

    fireEvent.click(screen.getByRole("button", { name: "复制代码" }));
    await waitFor(() =>
      expect(writeText).toHaveBeenCalledWith("const value = 1;"),
    );

    openExternal.mockRestore();
  });

  it("uses default expansion settings and allows manual collapse", async () => {
    const user = userEvent.setup();
    setSession2mdDefaultExpansion({
      defaultExpandThinking: true,
      defaultExpandTools: true,
      defaultExpandSystem: true,
    });
    setSessionFixtures(
      [
        {
          providerId: "dsh",
          sessionId: "expanded-session",
          title: "Expanded Session",
          sourcePath: "/mock/dsh/expanded-session.jsonl",
        },
      ],
      {
        "dsh:/mock/dsh/expanded-session.jsonl": [
          {
            role: "system",
            content: "system body\nsystem details",
            kind: "text",
          },
          { role: "assistant", content: "reasoning body", kind: "reasoning" },
          { role: "assistant", content: "answer body", kind: "text" },
          {
            role: "tool",
            content: '{"command":"pwd"}',
            kind: "toolCall",
            toolCallId: "call-expanded",
            toolName: "bash",
          },
        ],
      },
    );

    renderPage();

    expect(await screen.findByText(/system details/)).toBeInTheDocument();
    expect(screen.getByText("reasoning body")).toBeInTheDocument();
    expect(screen.getByText("$ pwd")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Thinking" }));
    await user.click(
      screen.getByRole("button", {
        name: /sessionManager\.roleTool · bash/,
      }),
    );
    await user.click(
      screen.getByRole("button", {
        name: "sessionManager.roleSystem · system body",
      }),
    );

    expect(screen.queryByText(/system details/)).not.toBeInTheDocument();
    expect(screen.queryByText("reasoning body")).not.toBeInTheDocument();
    expect(screen.queryByText("$ pwd")).not.toBeInTheDocument();
  });

  it("restores destructive controls without restoring terminal resume", async () => {
    renderPage();
    await screen.findByText("Alpha Session");

    expect(
      screen.getByRole("button", { name: "sessionManager.delete" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", {
        name: "sessionManager.manageBatchTooltip",
      }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /resume/i }),
    ).not.toBeInTheDocument();
  });

  it("hides deletion controls for read-only providers", async () => {
    setSessionFixtures(
      [
        {
          providerId: "qwen",
          sessionId: "qwen-session-1",
          title: "Qwen Session",
          projectDir: "/mock/qwen",
          lastActiveAt: 50,
          sourcePath: "/mock/qwen/session.jsonl",
          canDelete: false,
        },
      ],
      {
        "qwen:/mock/qwen/session.jsonl": [
          { role: "user", content: "hello", ts: 50 },
          { role: "assistant", content: "answer", ts: 51 },
        ],
      },
    );
    renderPage();
    fireEvent.click(
      await screen.findByRole("button", { name: /Qwen Session/i }),
    );
    expect(
      screen.queryByRole("button", { name: "sessionManager.delete" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", {
        name: "sessionManager.manageBatchTooltip",
      }),
    ).not.toBeInTheDocument();
  });

  it("exposes deletion for DSH sessions", async () => {
    setSessionFixtures(
      [
        {
          providerId: "dsh",
          sessionId: "dsh-session-1",
          title: "DSH Session",
          projectDir: "/mock/dsh",
          lastActiveAt: 50,
          sourcePath: "/mock/dsh/session.v4.jsonl",
          canDelete: true,
        },
      ],
      {
        "dsh:/mock/dsh/session.v4.jsonl": [
          { role: "user", content: "hello", ts: 50 },
          { role: "assistant", content: "answer", ts: 51 },
        ],
      },
    );
    renderPage();
    fireEvent.click(
      await screen.findByRole("button", { name: /DSH Session/i }),
    );

    expect(
      screen.getByRole("button", { name: "sessionManager.delete" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "sessionManager.manageBatchTooltip" }),
    ).toBeInTheDocument();
  });

  it("deletes the selected session after confirmation", async () => {
    const user = userEvent.setup();
    renderPage();

    await screen.findByText("Alpha Session");
    await user.click(screen.getByRole("button", { name: /Alpha Session/i }));
    await user.click(
      screen.getByRole("button", { name: "sessionManager.delete" }),
    );
    expect(
      screen.getByText("sessionManager.deleteConfirmMessage"),
    ).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", {
        name: "sessionManager.deleteConfirmAction",
      }),
    );

    await waitFor(() =>
      expect(screen.queryByText("Alpha Session")).not.toBeInTheDocument(),
    );
    expect(toastSuccessMock).toHaveBeenCalled();
  });

  it("reports when the backend did not delete the selected session", async () => {
    const user = userEvent.setup();
    const deleteSpy = vi
      .spyOn(sessionsApi, "delete")
      .mockResolvedValueOnce(false);
    renderPage();

    await screen.findByText("Alpha Session");
    await user.click(screen.getByRole("button", { name: /Alpha Session/i }));
    await user.click(
      screen.getByRole("button", { name: "sessionManager.delete" }),
    );
    await user.click(
      screen.getByRole("button", {
        name: "sessionManager.deleteConfirmAction",
      }),
    );

    await waitFor(() => expect(toastErrorMock).toHaveBeenCalled());
    expect(screen.getAllByText("Alpha Session")).not.toHaveLength(0);
    deleteSpy.mockRestore();
  });

  it("batch deletes selected sessions after confirmation", async () => {
    const user = userEvent.setup();
    renderPage();

    await screen.findByText("Alpha Session");
    await user.click(
      screen.getByRole("button", {
        name: "sessionManager.manageBatchTooltip",
      }),
    );
    await user.click(
      screen.getByRole("button", { name: "sessionManager.selectAllFiltered" }),
    );
    await user.click(
      screen.getByRole("button", { name: "sessionManager.deleteSelected" }),
    );
    expect(
      screen.getByText("sessionManager.batchDeleteConfirmMessage"),
    ).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", {
        name: "sessionManager.batchDeleteConfirmAction",
      }),
    );

    await waitFor(() => expect(toastSuccessMock).toHaveBeenCalled());
  });

  it("filters the all-session list by provider", async () => {
    const user = userEvent.setup();
    renderPage();
    const filter = await screen.findByRole("combobox", {
      name: "sessionManager.providerFilterTooltip",
    });
    await user.click(filter);
    await user.click(await screen.findByRole("option", { name: /Codex/i }));

    expect(screen.getAllByText("Alpha Session")).not.toHaveLength(0);
    expect(screen.queryByText("Claude Session")).not.toBeInTheDocument();
  });

  it("hides providers disabled in settings from the list and filter", async () => {
    setHiddenSessionProviders(["claude"]);
    const user = userEvent.setup();
    renderPage();

    expect(await screen.findByText("Alpha Session")).toBeInTheDocument();
    expect(screen.queryByText("Claude Session")).not.toBeInTheDocument();

    const filter = screen.getByRole("combobox", {
      name: "sessionManager.providerFilterTooltip",
    });
    await user.click(filter);
    expect(
      screen.queryByRole("option", { name: /Claude/i }),
    ).not.toBeInTheDocument();
  });

  it("filters to Pi sessions and exports their messages", async () => {
    const exportSpy = vi
      .spyOn(sessionsApi, "exportMarkdown")
      .mockResolvedValueOnce("/tmp/Pi Session.md");
    const user = userEvent.setup();
    renderPage();

    const filter = await screen.findByRole("combobox", {
      name: "sessionManager.providerFilterTooltip",
    });
    await user.click(filter);
    await user.click(await screen.findByRole("option", { name: /Pi/i }));

    expect(screen.getAllByText("Pi Session")).not.toHaveLength(0);
    expect(screen.queryByText("Alpha Session")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /Pi Session/i }));
    const exportButton = await screen.findByRole("button", {
      name: "sessionManager.export",
    });
    await waitFor(() => expect(exportButton).not.toBeDisabled());
    await user.click(exportButton);
    await user.click(
      await screen.findByRole("button", {
        name: "sessionManager.exportOptions.confirm",
      }),
    );

    await waitFor(() =>
      expect(exportSpy).toHaveBeenCalledWith(
        "Pi Session.md",
        "## User\n\npi prompt\n\n## Assistant\n\npi answer\n",
      ),
    );
  });

  it("virtualizes large session and message lists", async () => {
    const sessions: SessionMeta[] = Array.from({ length: 500 }, (_, index) => ({
      providerId: "codex",
      sessionId: `virtual-session-${index}`,
      title: `Session ${index}`,
      projectDir: `/mock/virtual-${index}`,
      lastActiveAt: 1_000 - index,
      sourcePath: `/mock/virtual/session-${index}.jsonl`,
    }));
    const selectedMessages: SessionMessage[] = Array.from(
      { length: 400 },
      (_, index) => ({
        role: index % 2 === 0 ? "user" : "assistant",
        content: `virtual message ${index}`,
        kind: "text",
      }),
    );
    setSessionFixtures(sessions, {
      "codex:/mock/virtual/session-0.jsonl": selectedMessages,
    });

    renderPage();

    expect(await screen.findByText("Session 0")).toBeInTheDocument();
    await waitFor(() =>
      expect(
        document.querySelectorAll('[data-session-row="session"]').length,
      ).toBeGreaterThan(0),
    );
    await waitFor(() =>
      expect(
        document.querySelectorAll("[data-message-index]").length,
      ).toBeGreaterThan(0),
    );

    expect(
      document.querySelectorAll('[data-session-row="session"]').length,
    ).toBeLessThan(sessions.length);
    expect(
      document.querySelectorAll("[data-message-index]").length,
    ).toBeLessThan(selectedMessages.length);
  });

  it("renders provider and directory groups as persisted collapsible sections", async () => {
    const user = userEvent.setup();
    const firstRender = renderPage();

    await screen.findByText("Alpha Session");
    await user.click(
      screen.getByRole("combobox", {
        name: "sessionManager.providerFilterTooltip",
      }),
    );
    await user.click(await screen.findByRole("option", { name: /Codex/i }));
    await switchToGroupedView(user);

    expect(
      screen.getByRole("button", {
        name: "sessionManager.collapseAllGroups",
      }),
    ).toBeInTheDocument();
    expect(
      screen.getAllByRole("button", {
        name: "sessionManager.toggleProviderGroup",
      }),
    ).toHaveLength(1);
    expect(
      screen.queryByRole("button", {
        name: "sessionManager.toggleDirectoryGroup",
      }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Alpha Session/ }),
    ).not.toBeInTheDocument();

    await user.click(
      screen.getAllByRole("button", {
        name: "sessionManager.toggleProviderGroup",
      })[0],
    );

    expect(
      screen.getAllByRole("button", {
        name: "sessionManager.toggleDirectoryGroup",
      }),
    ).toHaveLength(2);
    expect(
      screen.queryByRole("button", { name: /Alpha Session/ }),
    ).not.toBeInTheDocument();

    await user.click(
      screen.getAllByRole("button", {
        name: "sessionManager.toggleDirectoryGroup",
      })[0],
    );

    expect(
      screen.getByRole("button", { name: /Alpha Session/ }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /Codex Docs Session/ }),
    ).not.toBeInTheDocument();
    expect(
      JSON.parse(window.localStorage.getItem(GROUP_EXPANSION_STORAGE_KEY)!),
    ).toEqual({
      expandedProviderIds: ["codex"],
      expandedDirectoryKeys: ["codex:/mock/codex"],
    });

    firstRender.unmount();
    renderPage();

    await screen.findByRole("button", { name: /Alpha Session/ });
    expect(
      screen.getAllByRole("button", {
        name: "sessionManager.toggleDirectoryGroup",
      }),
    ).toHaveLength(2);

    await user.click(
      screen.getByRole("button", {
        name: "sessionManager.collapseAllGroups",
      }),
    );

    await waitFor(() =>
      expect(
        screen.queryByRole("button", {
          name: "sessionManager.toggleDirectoryGroup",
        }),
      ).not.toBeInTheDocument(),
    );
    expect(
      JSON.parse(window.localStorage.getItem(GROUP_EXPANSION_STORAGE_KEY)!),
    ).toEqual({
      expandedProviderIds: [],
      expandedDirectoryKeys: [],
    });
  });
});
