import { invoke } from "@tauri-apps/api/core";
import { useLayoutEffect } from "react";

export function MainWindowRevealer() {
  useLayoutEffect(() => {
    let cancelled = false;
    let showFrame = 0;

    const firstFrame = window.requestAnimationFrame(() => {
      showFrame = window.requestAnimationFrame(() => {
        if (cancelled) return;

        void invoke("show_main_window").catch((error) => {
          console.error("[startup] Failed to show the main window", error);
        });
      });
    });

    return () => {
      cancelled = true;
      window.cancelAnimationFrame(firstFrame);
      window.cancelAnimationFrame(showFrame);
    };
  }, []);

  return null;
}
