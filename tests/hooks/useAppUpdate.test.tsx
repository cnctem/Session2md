import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAppUpdate } from "@/hooks/useAppUpdate";
import type { AppUpdateInfo } from "@/lib/api/appUpdate";

const appUpdateApiMock = vi.hoisted(() => ({
  check: vi.fn(),
}));

vi.mock("@/lib/api/appUpdate", () => ({
  appUpdateApi: appUpdateApiMock,
}));

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn().mockResolvedValue("3.0.0"),
}));

const availableUpdate: AppUpdateInfo = {
  latestVersion: "3.1.0",
  releaseName: "Session2md 3.1.0",
  releaseNotes: "## Changes\n\n- Faster startup",
  releaseUrl: "https://github.com/cnctem/Session2md/releases/tag/v3.1.0",
  downloadUrl: "https://example.com/Session2md-3.1.0-macOS.dmg",
  publishedAt: "2026-10-08T00:00:00Z",
};

describe("useAppUpdate", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    appUpdateApiMock.check.mockResolvedValue(null);
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    delete (window as Window & { __TAURI_INTERNALS__?: unknown })
      .__TAURI_INTERNALS__;
  });

  it("checks automatically when the preference is enabled", async () => {
    renderHook(() => useAppUpdate(true));

    expect(appUpdateApiMock.check).not.toHaveBeenCalled();
    await act(async () => {
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
    });

    expect(appUpdateApiMock.check).toHaveBeenCalledTimes(1);
  });

  it("exposes an automatic update as a startup prompt", async () => {
    appUpdateApiMock.check.mockResolvedValue(availableUpdate);
    const { result } = renderHook(() => useAppUpdate(true));

    await act(async () => {
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
    });

    expect(result.current.updateInfo).toEqual(availableUpdate);
    expect(result.current.startupUpdateInfo).toEqual(availableUpdate);
  });

  it("does not check automatically when disabled or unset", async () => {
    const { rerender } = renderHook(
      ({ preference }: { preference: boolean | null }) =>
        useAppUpdate(preference),
      { initialProps: { preference: false as boolean | null } },
    );

    rerender({ preference: null });
    await act(async () => {
      vi.advanceTimersByTime(2000);
      await Promise.resolve();
    });

    expect(appUpdateApiMock.check).not.toHaveBeenCalled();
  });

  it("checks immediately when the preference changes from off to on", async () => {
    const { rerender } = renderHook(
      ({ preference }: { preference: boolean | null }) =>
        useAppUpdate(preference),
      { initialProps: { preference: false as boolean | null } },
    );

    rerender({ preference: true });
    await act(async () => {
      vi.advanceTimersByTime(0);
      await Promise.resolve();
    });

    expect(appUpdateApiMock.check).toHaveBeenCalledTimes(1);
  });

  it("keeps manual checks available when automatic checks are disabled", async () => {
    const { result } = renderHook(() => useAppUpdate(false));

    await act(async () => {
      await result.current.check();
    });

    expect(appUpdateApiMock.check).toHaveBeenCalledTimes(1);
  });

  it("does not open the startup prompt for a manual check", async () => {
    appUpdateApiMock.check.mockResolvedValue(availableUpdate);
    const { result } = renderHook(() => useAppUpdate(false));

    await act(async () => {
      await result.current.check();
    });

    expect(result.current.updateInfo).toEqual(availableUpdate);
    expect(result.current.startupUpdateInfo).toBeNull();
  });

  it("dismisses only the startup prompt and preserves update details", async () => {
    appUpdateApiMock.check.mockResolvedValue(availableUpdate);
    const { result } = renderHook(() => useAppUpdate(true));

    await act(async () => {
      vi.advanceTimersByTime(1000);
      await Promise.resolve();
    });
    act(() => result.current.dismissStartupUpdate());

    expect(result.current.startupUpdateInfo).toBeNull();
    expect(result.current.updateInfo).toEqual(availableUpdate);
  });
});
