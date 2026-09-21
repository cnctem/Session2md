import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SessionManagerPage } from "@/components/sessions/SessionManagerPage";
import { sessionsApi } from "@/lib/api/sessions";
import type { SessionMessage, SessionMeta } from "@/types";
import {
  setHiddenSessionProviders,
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

const renderPage = () => {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={client}>
      <SessionManagerPage />
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

    await waitFor(() =>
      expect(exportSpy).toHaveBeenCalledWith(
        "Alpha Session.md",
        "## User\n\nalpha\n",
      ),
    );
    expect(toastSuccessMock).toHaveBeenCalled();
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
            content: "hidden system instructions",
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
    expect(
      screen.queryByText("hidden system instructions"),
    ).not.toBeInTheDocument();

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
      screen.getByRole("button", { name: "sessionManager.roleSystem" }),
    );
    expect(
      await screen.findByText("hidden system instructions"),
    ).toBeInTheDocument();
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
          { role: "system", content: "system body", kind: "text" },
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

    expect(await screen.findByText("system body")).toBeInTheDocument();
    expect(screen.getByText("reasoning body")).toBeInTheDocument();
    expect(screen.getByText("$ pwd")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Thinking" }));
    await user.click(
      screen.getByRole("button", {
        name: /sessionManager\.roleTool · bash/,
      }),
    );
    await user.click(
      screen.getByRole("button", { name: "sessionManager.roleSystem" }),
    );

    expect(screen.queryByText("system body")).not.toBeInTheDocument();
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
          providerId: "dsh",
          sessionId: "dsh-session-1",
          title: "DSH Session",
          projectDir: "/mock/dsh",
          lastActiveAt: 50,
          sourcePath: "/mock/dsh/session.jsonl",
          canDelete: false,
        },
      ],
      {
        "dsh:/mock/dsh/session.jsonl": [
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
      screen.queryByRole("button", { name: "sessionManager.delete" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", {
        name: "sessionManager.manageBatchTooltip",
      }),
    ).not.toBeInTheDocument();
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
