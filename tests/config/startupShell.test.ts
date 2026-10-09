import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

const readProjectFile = (relativePath: string) =>
  readFileSync(path.resolve(process.cwd(), relativePath), "utf8");

describe("startup shell", () => {
  it("applies the saved theme before the application bundle loads", () => {
    const html = readProjectFile("src/index.html");
    const themeScript = html.match(/<script>(.*?)<\/script>/s)?.[1];
    const bundleIndex = html.indexOf('src="./main.tsx"');

    expect(themeScript).toContain("session2md-theme");
    expect(themeScript).toContain("prefers-color-scheme: dark");
    expect(html.indexOf("session2md-theme")).toBeLessThan(bundleIndex);
    expect(html).not.toContain("session2md-boot-splash");
  });

  it("keeps the CSP hash in sync with the inline theme script", () => {
    const html = readProjectFile("src/index.html");
    const themeScript = html.match(/<script>(.*?)<\/script>/s)?.[1];
    const config = JSON.parse(readProjectFile("src-tauri/tauri.conf.json")) as {
      app: { security: { csp: string } };
    };

    expect(themeScript).toBeDefined();
    const hash = createHash("sha256").update(themeScript!).digest("base64");
    expect(config.app.security.csp).toContain(`sha256-${hash}`);
  });

  it("keeps the main window hidden until the first frontend paint", () => {
    const config = JSON.parse(readProjectFile("src-tauri/tauri.conf.json")) as {
      app: { windows: Array<{ visible?: boolean }> };
    };
    const windowsConfig = JSON.parse(
      readProjectFile("src-tauri/tauri.windows.conf.json"),
    ) as {
      app: { windows: Array<{ visible?: boolean }> };
    };

    expect(config.app.windows[0].visible).toBe(false);
    expect(windowsConfig.app.windows[0].visible).toBe(false);
  });
});
