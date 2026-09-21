import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "@/App";
import { Session2mdSettingsPage } from "@/components/session-settings/Session2mdSettingsPage";
import { ThemeProvider } from "@/components/theme-provider";
import {
  SESSION_DIRECTORY_IDS,
  SESSION_PROVIDER_IDS,
} from "@/lib/sessionProviders";

const settingsApiMock = vi.hoisted(() => ({
  get: vi.fn(),
  save: vi.fn(),
}));

vi.mock("react-i18next", async (importOriginal) => {
  const actual = await importOriginal<typeof import("react-i18next")>();
  return {
    ...actual,
    useTranslation: () => ({
      t: (key: string) =>
        (
          ({
            "common.settings": "Settings",
            "sessionManager.title": "Session Manager",
            "sessionSettings.title": "Settings",
            "sessionSettings.backToSessions": "Back to Session Manager",
            "settings.general": "General",
            "settings.tabAdvanced": "Advanced",
            "settings.language": "Language",
            "settings.languageHint": "Choose the display language",
            "settings.languageOptionChinese": "Chinese",
            "settings.languageOptionTraditionalChinese": "Traditional Chinese",
            "settings.languageOptionEnglish": "English",
            "settings.languageOptionJapanese": "Japanese",
            "settings.theme": "Theme",
            "settings.themeHint": "Choose the appearance",
            "settings.themeLight": "Light",
            "settings.themeDark": "Dark",
            "settings.themeSystem": "System",
            "sessionSettings.directoryOverrides.title":
              "Configuration Directory Overrides",
            "sessionSettings.directoryOverrides.description":
              "Session-only paths",
            "sessionSettings.providerVisibility.title": "Agent Visibility",
            "sessionSettings.providerVisibility.description":
              "Choose visible agents",
            "sessionSettings.providerVisibility.selectAll": "Select all",
            "sessionSettings.providerVisibility.clearAll": "Clear all",
            "sessionSettings.exportContent.title": "Markdown Export Content",
            "sessionSettings.exportContent.description": "Export options",
            "sessionSettings.exportContent.includeThinking.label":
              "Include thinking",
            "sessionSettings.exportContent.includeThinking.description":
              "Include thinking description",
            "sessionSettings.exportContent.includeToolInputs.label":
              "Include tool arguments",
            "sessionSettings.exportContent.includeToolInputs.description":
              "Include arguments description",
            "sessionSettings.exportContent.includeToolOutputs.label":
              "Include tool output",
            "sessionSettings.exportContent.includeToolOutputs.description":
              "Include output description",
            "sessionSettings.directories.claude": "Claude",
            "sessionSettings.directories.codex": "Codex",
            "sessionSettings.directories.gemini": "Gemini",
            "sessionSettings.directories.grokbuild": "Grok Build",
            "sessionSettings.directories.opencode": "OpenCode",
            "sessionSettings.directories.openclaw": "OpenClaw",
            "sessionSettings.directories.hermes": "Hermes",
            "sessionSettings.directories.pi": "Pi",
            "sessionSettings.directories.dsh": "DeepSeek Harness",
            "sessionSettings.browseDirectory": "Choose directory",
            "sessionSettings.resetDirectory": "Restore default directory",
            "apps.codex": "Codex",
          }) as Record<string, string>
        )[key] ?? key,
    }),
  };
});

vi.mock("@/components/sessions/SessionManagerPage", () => ({
  SessionManagerPage: () => <div>session-manager</div>,
}));

vi.mock("@/components/theme-provider", () => ({
  ThemeProvider: ({ children }: { children: unknown }) => children,
  useTheme: () => ({ theme: "system", setTheme: vi.fn() }),
}));

vi.mock("@/lib/api/session2mdSettings", () => ({
  SESSION_DIRECTORY_IDS: [
    "claude",
    "codex",
    "gemini",
    "grokbuild",
    "opencode",
    "openclaw",
    "hermes",
    "pi",
  ],
  session2mdSettingsApi: settingsApiMock,
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

const snapshot = {
  directoryOverrides: {},
  hiddenProviders: [],
  exportThinking: false,
  exportToolInputs: false,
  exportToolOutputs: false,
  resolvedDirectories: Object.fromEntries(
    SESSION_DIRECTORY_IDS.map((id) => [id, `/home/mock/${id}`]),
  ) as Record<(typeof SESSION_DIRECTORY_IDS)[number], string>,
};

const renderWithProviders = (ui: ReactNode) => {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  });
  return render(
    <QueryClientProvider client={queryClient}>
      <ThemeProvider storageKey="session2md-theme-test">{ui}</ThemeProvider>
    </QueryClientProvider>,
  );
};

describe("Session2mdSettingsPage", () => {
  beforeEach(() => {
    window.localStorage.clear();
    settingsApiMock.get.mockResolvedValue(snapshot);
    settingsApiMock.save.mockImplementation(async (next) => ({
      ...snapshot,
      directoryOverrides: next.directoryOverrides,
      hiddenProviders: next.hiddenProviders,
      exportThinking: next.exportThinking,
      exportToolInputs: next.exportToolInputs,
      exportToolOutputs: next.exportToolOutputs,
    }));
  });

  it("opens from the header and returns to the session manager", async () => {
    renderWithProviders(<App />);

    expect(screen.getByText("session-manager")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Settings|设置/ }));

    expect(
      await screen.findByRole("tab", { name: /General|通用/ }),
    ).toBeInTheDocument();
    fireEvent.click(
      screen.getByRole("button", {
        name: /Back to Session Manager|返回会话管理/,
      }),
    );
    expect(screen.getByText("session-manager")).toBeInTheDocument();
  });

  it("stores the selected language under the Session2md key", async () => {
    renderWithProviders(<Session2mdSettingsPage />);

    await screen.findByRole("tab", { name: /General|通用/ });
    fireEvent.click(screen.getByRole("button", { name: "English" }));

    expect(window.localStorage.getItem("session2md-language")).toBe("en");
    expect(window.localStorage.getItem("language")).toBeNull();
  });

  it("saves a directory override and refreshes the session query", async () => {
    renderWithProviders(<Session2mdSettingsPage />);

    fireEvent.mouseDown(
      await screen.findByRole("tab", { name: /Advanced|高级/ }),
      { button: 0 },
    );
    const input = await screen.findByLabelText(/Codex/);
    fireEvent.change(input, { target: { value: "/Volumes/work/codex" } });
    fireEvent.blur(input);

    await waitFor(() =>
      expect(settingsApiMock.save).toHaveBeenCalledWith({
        directoryOverrides: { codex: "/Volumes/work/codex" },
        hiddenProviders: [],
        exportThinking: false,
        exportToolInputs: false,
        exportToolOutputs: false,
      }),
    );
  });

  it("saves hidden providers while preserving directory overrides", async () => {
    settingsApiMock.get.mockResolvedValue({
      ...snapshot,
      directoryOverrides: { codex: "/Volumes/work/codex" },
    });
    renderWithProviders(<Session2mdSettingsPage />);

    const codexVisibility = await screen.findByRole("checkbox", {
      name: "Codex",
    });
    expect(codexVisibility).toBeChecked();
    fireEvent.click(codexVisibility);

    await waitFor(() =>
      expect(settingsApiMock.save).toHaveBeenCalledWith({
        directoryOverrides: { codex: "/Volumes/work/codex" },
        hiddenProviders: ["codex"],
        exportThinking: false,
        exportToolInputs: false,
        exportToolOutputs: false,
      }),
    );
  });

  it("can hide and show all providers", async () => {
    renderWithProviders(<Session2mdSettingsPage />);

    fireEvent.click(await screen.findByRole("button", { name: "Clear all" }));
    await waitFor(() =>
      expect(settingsApiMock.save).toHaveBeenLastCalledWith({
        directoryOverrides: {},
        hiddenProviders: [...SESSION_PROVIDER_IDS],
        exportThinking: false,
        exportToolInputs: false,
        exportToolOutputs: false,
      }),
    );

    fireEvent.click(screen.getByRole("button", { name: "Select all" }));
    await waitFor(() =>
      expect(settingsApiMock.save).toHaveBeenLastCalledWith({
        directoryOverrides: {},
        hiddenProviders: [],
        exportThinking: false,
        exportToolInputs: false,
        exportToolOutputs: false,
      }),
    );
  });

  it("saves export content options independently", async () => {
    renderWithProviders(<Session2mdSettingsPage />);

    fireEvent.click(
      await screen.findByRole("switch", { name: /Include thinking/ }),
    );
    await waitFor(() =>
      expect(settingsApiMock.save).toHaveBeenLastCalledWith({
        directoryOverrides: {},
        hiddenProviders: [],
        exportThinking: true,
        exportToolInputs: false,
        exportToolOutputs: false,
      }),
    );
  });

  it("places export switches before agent visibility", async () => {
    renderWithProviders(<Session2mdSettingsPage />);

    const exportTitle = await screen.findByText("Markdown Export Content");
    const agentTitle = screen.getByText("Agent Visibility");

    expect(
      exportTitle.compareDocumentPosition(agentTitle) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      screen.getByRole("switch", { name: /Include tool arguments/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("switch", { name: /Include tool output/ }),
    ).toBeInTheDocument();
  });

  it("exposes the DeepSeek Harness source directory", async () => {
    renderWithProviders(<Session2mdSettingsPage />);
    fireEvent.mouseDown(
      await screen.findByRole("tab", { name: /Advanced|高级/ }),
      { button: 0 },
    );
    expect(await screen.findByLabelText("DeepSeek Harness")).toHaveValue(
      "/home/mock/dsh",
    );
  });
});
