import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useAppUpdate } from "@/hooks/useAppUpdate";

const appUpdateApiMock = vi.hoisted(() => ({
  check: vi.fn(),
}));

vi.mock("@/lib/api/appUpdate", () => ({
  appUpdateApi: appUpdateApiMock,
}));

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn().mockResolvedValue("3.0.0"),
}));

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
});
