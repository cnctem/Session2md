import { useCallback, useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { appUpdateApi, type AppUpdateInfo } from "@/lib/api/appUpdate";
import { extractErrorMessage } from "@/utils/errorUtils";

export interface AppUpdateController {
  currentVersion: string;
  updateInfo: AppUpdateInfo | null;
  startupUpdateInfo: AppUpdateInfo | null;
  isChecking: boolean;
  hasChecked: boolean;
  error: string | null;
  check: () => Promise<AppUpdateInfo | null>;
  dismissStartupUpdate: () => void;
}

export function useAppUpdate(
  autoCheckUpdates: boolean | null | undefined,
): AppUpdateController {
  const [currentVersion, setCurrentVersion] = useState("");
  const [updateInfo, setUpdateInfo] = useState<AppUpdateInfo | null>(null);
  const [startupUpdateInfo, setStartupUpdateInfo] =
    useState<AppUpdateInfo | null>(null);
  const [isChecking, setIsChecking] = useState(false);
  const [hasChecked, setHasChecked] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const checkingRef = useRef(false);
  const previousAutoCheckRef = useRef<boolean | null | undefined>(undefined);

  useEffect(() => {
    // Renderer-only tests and browser previews do not expose the Tauri bridge.
    // Keep this hook inert there instead of showing a failed update check.
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
      return;
    }

    let active = true;
    void getVersion()
      .then((version) => {
        if (active) setCurrentVersion(version);
      })
      .catch((versionError) => {
        console.warn("[Session2md] Failed to read app version", versionError);
      });

    return () => {
      active = false;
    };
  }, []);

  const runCheck = useCallback(
    async (showStartupPrompt: boolean): Promise<AppUpdateInfo | null> => {
      if (checkingRef.current) return null;

      checkingRef.current = true;
      setIsChecking(true);
      setError(null);

      try {
        const result = await appUpdateApi.check();
        setUpdateInfo(result);
        setHasChecked(true);
        if (showStartupPrompt) {
          setStartupUpdateInfo(result);
        }
        return result;
      } catch (checkError) {
        const message =
          extractErrorMessage(checkError) ||
          (checkError instanceof Error
            ? checkError.message
            : String(checkError));
        setError(message);
        setHasChecked(true);
        if (showStartupPrompt) {
          setStartupUpdateInfo(null);
        }
        throw checkError;
      } finally {
        checkingRef.current = false;
        setIsChecking(false);
      }
    },
    [],
  );

  const check = useCallback(() => runCheck(false), [runCheck]);

  const checkForStartup = useCallback(() => runCheck(true), [runCheck]);

  const dismissStartupUpdate = useCallback(() => {
    setStartupUpdateInfo(null);
  }, []);

  useEffect(() => {
    const previousAutoCheck = previousAutoCheckRef.current;
    previousAutoCheckRef.current = autoCheckUpdates;

    if (
      autoCheckUpdates !== true ||
      typeof window === "undefined" ||
      !("__TAURI_INTERNALS__" in window)
    ) {
      return;
    }

    const delay =
      previousAutoCheck === null || previousAutoCheck === false ? 0 : 1000;
    const timer = window.setTimeout(() => {
      void checkForStartup().catch((checkError) => {
        console.warn("[Session2md] Automatic update check failed", checkError);
      });
    }, delay);

    return () => window.clearTimeout(timer);
  }, [autoCheckUpdates, checkForStartup]);

  return {
    currentVersion,
    updateInfo,
    startupUpdateInfo,
    isChecking,
    hasChecked,
    error,
    check,
    dismissStartupUpdate,
  };
}
