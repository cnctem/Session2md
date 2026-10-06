import { describe, expect, it } from "vitest";
import { hasIcon } from "@/icons/extracted";

describe("session provider icons", () => {
  it("registers icons for every newly supported provider", () => {
    for (const id of [
      "cursor",
      "antigravity",
      "reasonix",
      "mimocode",
      "deveco",
      "zcode",
      "kimi",
      "kilocode",
      "qoder",
      "workbuddy",
      "qwen",
      "continue",
      "cline",
      "goose",
      "zed",
      "crush",
      "teleagent",
      "deepseek",
    ]) {
      expect(hasIcon(id), id).toBe(true);
    }
  });
});
