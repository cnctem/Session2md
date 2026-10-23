import { describe, expect, it } from "vitest";
import {
  classifyMarkdownLink,
  decodeLocalPath,
  getPathBasename,
} from "@/lib/markdown/localPaths";

describe("classifyMarkdownLink", () => {
  it("normalizes http and https URLs", () => {
    expect(classifyMarkdownLink("https://example.com/docs")).toEqual({
      kind: "external",
      url: "https://example.com/docs",
    });
    expect(classifyMarkdownLink("http://example.com")).toEqual({
      kind: "external",
      url: "http://example.com/",
    });
  });

  it("treats relative paths as local files", () => {
    expect(
      classifyMarkdownLink("video/cinema/output/智解文境-封面.png"),
    ).toEqual({
      kind: "local",
      path: "video/cinema/output/智解文境-封面.png",
    });
    expect(classifyMarkdownLink("./docs/deck/汇报.pdf")).toEqual({
      kind: "local",
      path: "./docs/deck/汇报.pdf",
    });
  });

  it("recognizes absolute, home and file URLs as local", () => {
    expect(
      classifyMarkdownLink(
        "/Users/a11111/code/wic/docs/deck/智解文境-作品汇报.pdf",
      ),
    ).toEqual({
      kind: "local",
      path: "/Users/a11111/code/wic/docs/deck/智解文境-作品汇报.pdf",
    });
    expect(classifyMarkdownLink("~/notes.md")).toMatchObject({
      kind: "local",
    });
    expect(classifyMarkdownLink("file:///tmp/a.png")).toMatchObject({
      kind: "local",
    });
    expect(
      classifyMarkdownLink("C:\\Users\\me\\cover.png", { platform: "windows" }),
    ).toMatchObject({ kind: "local" });
  });

  it("rejects unsupported schemes and document anchors", () => {
    for (const value of [
      "javascript:alert(1)",
      "data:text/html,hi",
      "mailto:user@example.com",
      "#section",
      "//example.com/a.png",
    ]) {
      expect(classifyMarkdownLink(value).kind).toBe("unsupported");
    }
  });

  it("treats bare www hosts as https links", () => {
    expect(classifyMarkdownLink("www.example.com/docs")).toEqual({
      kind: "external",
      url: "https://www.example.com/docs",
    });
  });
});

describe("decodeLocalPath", () => {
  it("decodes URI-encoded path segments exactly once", () => {
    expect(decodeLocalPath("docs/assets/%E7%AF%87%E7%9B%AE%E5%BA%93.png")).toBe(
      "docs/assets/篇目库.png",
    );
    expect(decodeLocalPath("docs/cabin%20library.png")).toBe(
      "docs/cabin library.png",
    );
  });

  it("keeps file URLs and malformed escapes untouched", () => {
    expect(decodeLocalPath("file:///tmp/a%20b.png")).toBe(
      "file:///tmp/a%20b.png",
    );
    expect(decodeLocalPath("docs/100%.png")).toBe("docs/100%.png");
  });
});

describe("getPathBasename", () => {
  it("returns the last path segment for posix and windows paths", () => {
    expect(getPathBasename("/tmp/docs/cover.png")).toBe("cover.png");
    expect(getPathBasename("C:\\docs\\cover.png")).toBe("cover.png");
    expect(getPathBasename("cover.png")).toBe("cover.png");
  });
});
