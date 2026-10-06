import { describe, expect, it } from "vitest";
import { SESSION_PROVIDER_ICON_ASSETS } from "@/components/sessions/sessionProviderIcons";
import { SESSION_PROVIDER_IDS } from "@/lib/sessionProviders";

describe("session provider icons", () => {
  it("registers exactly one valid icon for every session provider", () => {
    expect(Object.keys(SESSION_PROVIDER_ICON_ASSETS).sort()).toEqual(
      [...SESSION_PROVIDER_IDS].sort(),
    );

    for (const id of SESSION_PROVIDER_IDS) {
      const asset = SESSION_PROVIDER_ICON_ASSETS[id];
      expect(asset.name, id).not.toBe("");

      if (asset.kind === "svg") {
        expect(asset.content, id).toMatch(/<svg[\s>]/);
      } else {
        expect(asset.url, id).not.toBe("");
      }
    }
  });
});
