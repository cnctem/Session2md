import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AppUpdateDialog } from "@/components/session-settings/AppUpdateDialog";
import type { AppUpdateInfo } from "@/lib/api/appUpdate";

const sessionsApiMock = vi.hoisted(() => ({
  openExternalUrl: vi.fn(),
}));

vi.mock("@/lib/api/sessions", () => ({
  sessionsApi: sessionsApiMock,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    i18n: { resolvedLanguage: "en", language: "en" },
    t: (key: string, options?: Record<string, unknown>) => {
      const translations: Record<string, string> = {
        "sessionSettings.updateDialog.title":
          "Session2md {{version}} is available",
        "sessionSettings.updateDialog.description": "A new version is ready.",
        "sessionSettings.updateDialog.currentVersion": "Current version",
        "sessionSettings.updateDialog.latestVersion": "Latest version",
        "sessionSettings.updateDialog.releaseNotes": "Release notes",
        "sessionSettings.updateDialog.later": "Later",
        "sessionSettings.updateDialog.close": "Close update dialog",
        "sessionSettings.about.updates.publishedAt": "Published {{date}}",
        "sessionSettings.about.updates.viewRelease": "View release",
        "sessionSettings.about.updates.download": "Download {{version}}",
      };
      let value =
        translations[key] ??
        (typeof options?.defaultValue === "string"
          ? options.defaultValue
          : key);
      for (const [name, replacement] of Object.entries(options ?? {})) {
        value = value.split(`{{${name}}}`).join(String(replacement));
      }
      return value;
    },
  }),
}));

const availableUpdate: AppUpdateInfo = {
  latestVersion: "3.1.0",
  releaseName: "Session2md 3.1.0",
  releaseNotes:
    "## Changes\n\n- Faster startup\n\n[Details](https://example.com/details)",
  releaseUrl: "https://github.com/cnctem/Session2md/releases/tag/v3.1.0",
  downloadUrl: "https://example.com/Session2md-3.1.0-macOS.dmg",
  publishedAt: "2026-10-08T00:00:00Z",
};

describe("AppUpdateDialog", () => {
  beforeEach(() => {
    sessionsApiMock.openExternalUrl.mockResolvedValue(true);
  });

  it("renders version details and Markdown release notes", () => {
    render(
      <AppUpdateDialog
        update={availableUpdate}
        currentVersion="3.0.0"
        onDismiss={vi.fn()}
      />,
    );

    expect(
      screen.getByText("Session2md v3.1.0 is available"),
    ).toBeInTheDocument();
    expect(screen.getByText("3.0.0")).toBeInTheDocument();
    expect(screen.getByText("v3.1.0")).toBeInTheDocument();
    expect(screen.getByText("Session2md 3.1.0")).toBeInTheDocument();
    expect(screen.getByText("Release notes")).toBeInTheDocument();
    expect(
      screen.getByRole("heading", { name: "Changes" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Faster startup")).toBeInTheDocument();
  });

  it("opens the download and release URLs externally", async () => {
    render(
      <AppUpdateDialog
        update={availableUpdate}
        currentVersion="3.0.0"
        onDismiss={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Download v3.1.0" }));
    await waitFor(() =>
      expect(sessionsApiMock.openExternalUrl).toHaveBeenCalledWith(
        availableUpdate.downloadUrl,
      ),
    );

    fireEvent.click(screen.getByRole("button", { name: "View release" }));
    await waitFor(() =>
      expect(sessionsApiMock.openExternalUrl).toHaveBeenCalledWith(
        availableUpdate.releaseUrl,
      ),
    );
  });

  it("closes from the later action and omits an empty release-notes section", () => {
    const onDismiss = vi.fn();
    render(
      <AppUpdateDialog
        update={{ ...availableUpdate, releaseNotes: "  " }}
        currentVersion="3.0.0"
        onDismiss={onDismiss}
      />,
    );

    expect(screen.queryByText("Release notes")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Later" }));
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });

  it("renders nothing without update information", () => {
    const { container } = render(
      <AppUpdateDialog
        update={null}
        currentVersion="3.0.0"
        onDismiss={vi.fn()}
      />,
    );

    expect(container).toBeEmptyDOMElement();
  });
});
