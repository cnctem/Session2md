import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SessionManagerPage } from "@/components/sessions/SessionManagerPage";
import { sessionsApi } from "@/lib/api/sessions";
import type { SessionMessage, SessionMeta } from "@/types";
import { setSessionFixtures } from "../msw/state";

const toastSuccessMock = vi.fn();
const toastErrorMock = vi.fn();

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

    fireEvent.click(await screen.findByRole("button", { name: /Alpha Session/i }));
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

  it("has no destructive or terminal-resume controls", async () => {
    renderPage();
    await screen.findByText("Alpha Session");

    expect(screen.queryByRole("button", { name: /delete/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /resume/i })).not.toBeInTheDocument();
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
});
