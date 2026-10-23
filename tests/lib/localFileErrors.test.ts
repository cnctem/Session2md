import { describe, expect, it } from "vitest";

import {
  describeLocalFileError,
  parseLocalFileError,
} from "@/lib/markdown/localFileErrors";

const translate = (key: string, options?: Record<string, unknown>) => {
  const fallback = options?.defaultValue;
  const template = typeof fallback === "string" ? fallback : key;
  return template.replace(/\{\{(\w+)\}\}/g, (_, name: string) =>
    String(options?.[name] ?? ""),
  );
};

describe("parseLocalFileError", () => {
  it("reads structured command errors", () => {
    expect(
      parseLocalFileError({
        code: "fileNotFound",
        message: "File not found: /tmp/a.png",
        path: "/tmp/a.png",
      }),
    ).toEqual({
      code: "fileNotFound",
      message: "File not found: /tmp/a.png",
      path: "/tmp/a.png",
    });
  });

  it("keeps plain string and Error messages working", () => {
    expect(parseLocalFileError("boom")).toEqual({ message: "boom" });
    expect(parseLocalFileError(new Error("failed"))).toEqual({
      message: "failed",
    });
  });

  it("returns an empty message for unknown values", () => {
    expect(parseLocalFileError(undefined)).toEqual({ message: "" });
  });
});

describe("describeLocalFileError", () => {
  it("shows the resolved path and a hint for missing files", () => {
    const result = describeLocalFileError(
      {
        code: "fileNotFound",
        message: "File not found: /tmp/wic/docs/missing.png",
        path: "/tmp/wic/docs/missing.png",
      },
      translate,
    );

    expect(result.title).toBe("File not found: /tmp/wic/docs/missing.png");
    expect(result.description).toContain("moved or deleted");
  });

  it("falls back to the generic open failure for other errors", () => {
    expect(
      describeLocalFileError(
        { code: "readFailed", message: "Permission denied" },
        translate,
      ),
    ).toEqual({ title: "Failed to open file: Permission denied" });
  });

  it("uses a fallback for unknown errors", () => {
    expect(describeLocalFileError(undefined, translate).title).toBe(
      "Failed to open file: Unknown error",
    );
  });
});
