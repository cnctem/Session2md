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
            "common.about": "About",
            "common.version": "Version",
            "common.loading": "Loading",
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
            "sessionSettings.autoUpdatePrompt.title": "Automatic Update Checks",
            "sessionSettings.autoUpdatePrompt.description":
              "Allow automatic update checks",
            "sessionSettings.autoUpdatePrompt.yes": "Yes, check automatically",
            "sessionSettings.autoUpdatePrompt.no":
              "No, don't check automatically",
            "sessionSettings.autoUpdatePrompt.saveFailed":
              "Save auto update preference failed",
            "sessionSettings.providerVisibility.title": "Agent Visibility",
            "sessionSettings.providerVisibility.description":
              "Choose visible agents",
            "sessionSettings.providerVisibility.selectAll": "Select all",
            "sessionSettings.providerVisibility.clearAll": "Clear all",
            "sessionSettings.defaultExpansion.title": "Default Expansion",
            "sessionSettings.defaultExpansion.description":
              "Default expansion options",
            "sessionSettings.defaultExpansion.saveFailed":
              "Save expansion failed",
            "sessionSettings.defaultExpansion.expandThinking.label":
              "Expand thinking by default",
            "sessionSettings.defaultExpansion.expandThinking.description":
              "Expand thinking description",
            "sessionSettings.defaultExpansion.expandTools.label":
              "Expand tools by default",
            "sessionSettings.defaultExpansion.expandTools.description":
              "Expand tools description",
            "sessionSettings.defaultExpansion.expandSystem.label":
              "Expand system by default",
            "sessionSettings.defaultExpansion.expandSystem.description":
              "Expand system description",
            "sessionSettings.messagePreview.title": "Message Preview",
            "sessionSettings.messagePreview.description":
              "Message preview options",
            "sessionSettings.messagePreview.saveFailed":
              "Save message preview failed",
            "sessionSettings.messagePreview.renderMarkdown.label":
              "Render Markdown",
            "sessionSettings.messagePreview.renderMarkdown.description":
              "Render markdown description",
            "sessionSettings.exportContent.title": "Markdown Export Content",
            "sessionSettings.exportContent.description": "Export options",
            "sessionSettings.exportContent.promptBeforeExport.label":
              "Ask before every export",
            "sessionSettings.exportContent.promptBeforeExport.description":
              "Ask before export description",
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
            "sessionSettings.about.title": "About Session2md",
            "sessionSettings.about.description": "App information",
            "sessionSettings.about.appDescription": "Session2md summary",
            "sessionSettings.about.localFirstDescription":
              "Local-first summary",
            "sessionSettings.about.github": "GitHub",
            "sessionSettings.about.releases": "Releases",
            "sessionSettings.about.notChecked": "Updates have not been checked",
            "sessionSettings.about.updates.title": "Software Updates",
            "sessionSettings.about.updates.description": "Update description",
            "sessionSettings.about.updates.checkNow": "Check for updates",
            "sessionSettings.about.updates.autoCheck.label":
              "Automatically check for updates",
            "sessionSettings.about.updates.autoCheck.description":
              "Check GitHub Releases when the app starts",
            "sessionSettings.about.updates.autoCheck.saveFailed":
              "Save auto update preference failed",
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
  autoCheckUpdates: true,
  promptBeforeExport: true,
  exportThinking: false,
  exportToolInputs: false,
  exportToolOutputs: false,
  defaultExpandThinking: false,
  defaultExpandTools: false,
  defaultExpandSystem: false,
  renderMarkdown: true,
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
      autoCheckUpdates: next.autoCheckUpdates,
      promptBeforeExport: next.promptBeforeExport,
      exportThinking: next.exportThinking,
      exportToolInputs: next.exportToolInputs,
      exportToolOutputs: next.exportToolOutputs,
      defaultExpandThinking: next.defaultExpandThinking,
      defaultExpandTools: next.defaultExpandTools,
      defaultExpandSystem: next.defaultExpandSystem,
      renderMarkdown: next.renderMarkdown,
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

  it("asks for the automatic update preference only when it is unset", async () => {
    settingsApiMock.get.mockResolvedValue({
      ...snapshot,
      autoCheckUpdates: null,
    });
    renderWithProviders(<App />);

    expect(
      await screen.findByText("Automatic Update Checks"),
    ).toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    fireEvent.click(
      screen.getByRole("button", { name: "Yes, check automatically" }),
    );

    await waitFor(() =>
      expect(settingsApiMock.save).toHaveBeenCalledWith({
        directoryOverrides: {},
        hiddenProviders: [],
        autoCheckUpdates: true,
        promptBeforeExport: true,
        exportThinking: false,
        exportToolInputs: false,
        exportToolOutputs: false,
        defaultExpandThinking: false,
        defaultExpandTools: false,
        defaultExpandSystem: false,
        renderMarkdown: true,
      }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
  });

  it("stores an explicit no for automatic update checks", async () => {
    settingsApiMock.get.mockResolvedValue({
      ...snapshot,
      autoCheckUpdates: null,
    });
    renderWithProviders(<App />);

    fireEvent.click(
      await screen.findByRole("button", {
        name: "No, don't check automatically",
      }),
    );

    await waitFor(() =>
      expect(settingsApiMock.save).toHaveBeenCalledWith(
        expect.objectContaining({ autoCheckUpdates: false }),
      ),
    );
  });

  it("keeps the automatic update prompt open when saving fails", async () => {
    settingsApiMock.get.mockResolvedValue({
      ...snapshot,
      autoCheckUpdates: null,
    });
    settingsApiMock.save.mockRejectedValueOnce(new Error("save failed"));
    renderWithProviders(<App />);

    fireEvent.click(
      await screen.findByRole("button", {
        name: "Yes, check automatically",
      }),
    );

    await waitFor(() =>
      expect(settingsApiMock.save).toHaveBeenCalledWith(
        expect.objectContaining({ autoCheckUpdates: true }),
      ),
    );
    expect(screen.getByRole("dialog")).toBeInTheDocument();
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
        autoCheckUpdates: true,
        promptBeforeExport: true,
        exportThinking: false,
        exportToolInputs: false,
        exportToolOutputs: false,
        defaultExpandThinking: false,
        defaultExpandTools: false,
        defaultExpandSystem: false,
        renderMarkdown: true,
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
        autoCheckUpdates: true,
        promptBeforeExport: true,
        exportThinking: false,
        exportToolInputs: false,
        exportToolOutputs: false,
        defaultExpandThinking: false,
        defaultExpandTools: false,
        defaultExpandSystem: false,
        renderMarkdown: true,
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
        autoCheckUpdates: true,
        promptBeforeExport: true,
        exportThinking: false,
        exportToolInputs: false,
        exportToolOutputs: false,
        defaultExpandThinking: false,
        defaultExpandTools: false,
        defaultExpandSystem: false,
        renderMarkdown: true,
      }),
    );

    fireEvent.click(screen.getByRole("button", { name: "Select all" }));
    await waitFor(() =>
      expect(settingsApiMock.save).toHaveBeenLastCalledWith({
        directoryOverrides: {},
        hiddenProviders: [],
        autoCheckUpdates: true,
        promptBeforeExport: true,
        exportThinking: false,
        exportToolInputs: false,
        exportToolOutputs: false,
        defaultExpandThinking: false,
        defaultExpandTools: false,
        defaultExpandSystem: false,
        renderMarkdown: true,
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
        autoCheckUpdates: true,
        promptBeforeExport: true,
        exportThinking: true,
        exportToolInputs: false,
        exportToolOutputs: false,
        defaultExpandThinking: false,
        defaultExpandTools: false,
        defaultExpandSystem: false,
        renderMarkdown: true,
      }),
    );
  });

  it("defaults prompt before export on and saves it independently", async () => {
    renderWithProviders(<Session2mdSettingsPage />);

    const promptBeforeExport = await screen.findByRole("switch", {
      name: "Ask before every export",
    });
    expect(promptBeforeExport).toBeChecked();

    fireEvent.click(promptBeforeExport);
    await waitFor(() =>
      expect(settingsApiMock.save).toHaveBeenLastCalledWith({
        directoryOverrides: {},
        hiddenProviders: [],
        autoCheckUpdates: true,
        promptBeforeExport: false,
        exportThinking: false,
        exportToolInputs: false,
        exportToolOutputs: false,
        defaultExpandThinking: false,
        defaultExpandTools: false,
        defaultExpandSystem: false,
        renderMarkdown: true,
      }),
    );
  });

  it("saves default expansion options independently", async () => {
    renderWithProviders(<Session2mdSettingsPage />);

    fireEvent.click(
      await screen.findByRole("switch", {
        name: "Expand thinking by default",
      }),
    );
    await waitFor(() =>
      expect(settingsApiMock.save).toHaveBeenLastCalledWith({
        directoryOverrides: {},
        hiddenProviders: [],
        autoCheckUpdates: true,
        promptBeforeExport: true,
        exportThinking: false,
        exportToolInputs: false,
        exportToolOutputs: false,
        defaultExpandThinking: true,
        defaultExpandTools: false,
        defaultExpandSystem: false,
        renderMarkdown: true,
      }),
    );
  });

  it("defaults Markdown rendering on and saves it independently", async () => {
    renderWithProviders(<Session2mdSettingsPage />);

    const renderMarkdown = await screen.findByRole("switch", {
      name: "Render Markdown",
    });
    expect(renderMarkdown).toBeChecked();

    fireEvent.click(renderMarkdown);
    await waitFor(() =>
      expect(settingsApiMock.save).toHaveBeenLastCalledWith({
        directoryOverrides: {},
        hiddenProviders: [],
        autoCheckUpdates: true,
        promptBeforeExport: true,
        exportThinking: false,
        exportToolInputs: false,
        exportToolOutputs: false,
        defaultExpandThinking: false,
        defaultExpandTools: false,
        defaultExpandSystem: false,
        renderMarkdown: false,
      }),
    );
  });

  it("places export switches before agent visibility", async () => {
    renderWithProviders(<Session2mdSettingsPage />);

    const exportTitle = await screen.findByText("Markdown Export Content");
    const messagePreviewTitle = screen.getByText("Message Preview");
    const defaultExpansionTitle = screen.getByText("Default Expansion");
    const agentTitle = screen.getByText("Agent Visibility");

    expect(
      messagePreviewTitle.compareDocumentPosition(defaultExpansionTitle) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      defaultExpansionTitle.compareDocumentPosition(exportTitle) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      exportTitle.compareDocumentPosition(agentTitle) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    const promptBeforeExport = screen.getByRole("switch", {
      name: "Ask before every export",
    });
    const includeThinking = screen.getByRole("switch", {
      name: /Include thinking/,
    });
    expect(
      promptBeforeExport.compareDocumentPosition(includeThinking) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      screen.getByRole("switch", { name: /Include tool arguments/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("switch", { name: /Include tool output/ }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("switch", { name: "Expand tools by default" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("switch", { name: "Expand system by default" }),
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

  it("shows the About section as the third settings tab", async () => {
    renderWithProviders(<Session2mdSettingsPage />);

    fireEvent.mouseDown(await screen.findByRole("tab", { name: "About" }), {
      button: 0,
    });

    expect(await screen.findByText("About Session2md")).toBeInTheDocument();
    expect(screen.getByText("Software Updates")).toBeInTheDocument();
  });

  it("changes automatic update checks from the About section", async () => {
    renderWithProviders(<Session2mdSettingsPage />);

    fireEvent.mouseDown(await screen.findByRole("tab", { name: "About" }), {
      button: 0,
    });
    const autoCheckSwitch = await screen.findByRole("switch", {
      name: "Automatically check for updates",
    });
    expect(autoCheckSwitch).toBeChecked();

    fireEvent.click(autoCheckSwitch);

    await waitFor(() =>
      expect(settingsApiMock.save).toHaveBeenCalledWith(
        expect.objectContaining({ autoCheckUpdates: false }),
      ),
    );
  });

  it("rolls back the About switch when saving fails", async () => {
    settingsApiMock.save.mockRejectedValueOnce(new Error("save failed"));
    renderWithProviders(<Session2mdSettingsPage />);

    fireEvent.mouseDown(await screen.findByRole("tab", { name: "About" }), {
      button: 0,
    });
    const autoCheckSwitch = await screen.findByRole("switch", {
      name: "Automatically check for updates",
    });
    fireEvent.click(autoCheckSwitch);

    await waitFor(() => expect(autoCheckSwitch).toBeChecked());
  });
});
