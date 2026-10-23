import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { MarkdownMessageContent } from "@/components/sessions/MarkdownMessageContent";
import { clearMarkdownImageCache } from "@/components/sessions/MarkdownImage";

const localFilesMock = vi.hoisted(() => ({
  openPath: vi.fn(),
  readImage: vi.fn(),
}));

const toastMock = vi.hoisted(() => ({
  error: vi.fn(),
  success: vi.fn(),
}));

vi.mock("@/lib/api/localFiles", () => ({
  localFilesApi: localFilesMock,
}));

vi.mock("sonner", () => ({
  toast: toastMock,
}));

describe("MarkdownMessageContent", () => {
  beforeEach(() => {
    clearMarkdownImageCache();
    localFilesMock.openPath.mockResolvedValue("/tmp/file");
    localFilesMock.readImage.mockReset();
  });

  it("renders basic Markdown and preserves single line breaks", () => {
    const { container } = render(
      <MarkdownMessageContent
        content={[
          "# Heading",
          "",
          "- first",
          "- second",
          "",
          "> quoted",
          "",
          "A [link](https://example.com/docs) with `inline code`.",
          "",
          "first line",
          "second line",
        ].join("\n")}
        onCopyCode={vi.fn()}
        onOpenLink={vi.fn()}
      />,
    );

    expect(
      screen.getByRole("heading", { level: 1, name: "Heading" }),
    ).toBeInTheDocument();
    expect(screen.getByText("first")).toBeInTheDocument();
    expect(screen.getByText("second")).toBeInTheDocument();
    expect(screen.getByText("quoted")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "link" })).toHaveAttribute(
      "href",
      "https://example.com/docs",
    );
    expect(screen.getByText("inline code").tagName).toBe("CODE");
    expect(container.querySelectorAll("br").length).toBeGreaterThan(0);
  });

  it("opens only rendered links through the callback", () => {
    const onOpenLink = vi.fn();
    render(
      <MarkdownMessageContent
        content="[safe](https://example.com) [unsafe](javascript:alert(1))"
        onCopyCode={vi.fn()}
        onOpenLink={onOpenLink}
      />,
    );

    fireEvent.click(screen.getByRole("link", { name: "safe" }));

    expect(onOpenLink).toHaveBeenCalledWith("https://example.com/");
    expect(
      screen.queryByRole("link", { name: "unsafe" }),
    ).not.toBeInTheDocument();
  });

  it("opens absolute and relative local links against the session directory", () => {
    const onOpenLink = vi.fn();
    render(
      <MarkdownMessageContent
        content={[
          "[封面](video/cinema/output/智解文境-动态版-封面.png)",
          "[PDF 版](/Users/a11111/code/wic/docs/deck/智解文境-作品汇报.pdf)",
        ].join("\n\n")}
        baseDir="/Users/a11111/code/wic"
        onCopyCode={vi.fn()}
        onOpenLink={onOpenLink}
      />,
    );

    fireEvent.click(screen.getByRole("link", { name: "封面" }));
    fireEvent.click(screen.getByRole("link", { name: "PDF 版" }));

    expect(localFilesMock.openPath).toHaveBeenNthCalledWith(
      1,
      "video/cinema/output/智解文境-动态版-封面.png",
      "/Users/a11111/code/wic",
    );
    expect(localFilesMock.openPath).toHaveBeenNthCalledWith(
      2,
      "/Users/a11111/code/wic/docs/deck/智解文境-作品汇报.pdf",
      "/Users/a11111/code/wic",
    );
    expect(onOpenLink).not.toHaveBeenCalled();
  });

  it("shows a localized hint when a linked file is missing", async () => {
    localFilesMock.openPath.mockRejectedValue({
      code: "fileNotFound",
      message: "File not found: /tmp/session/docs/missing.pdf",
      path: "/tmp/session/docs/missing.pdf",
    });

    render(
      <MarkdownMessageContent
        content="[汇报 PDF](docs/missing.pdf)"
        baseDir="/tmp/session"
        onCopyCode={vi.fn()}
        onOpenLink={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("link", { name: "汇报 PDF" }));

    await waitFor(() => expect(toastMock.error).toHaveBeenCalledTimes(1));
    expect(toastMock.error).toHaveBeenCalledWith(
      "File not found: /tmp/session/docs/missing.pdf",
      {
        description: expect.stringContaining("moved or deleted"),
      },
    );
  });

  it("renders relative images inline through the local file reader", async () => {
    localFilesMock.readImage.mockResolvedValue({
      path: "/Users/a11111/code/wic/docs/assets/cabin-library.png",
      mime: "image/png",
      dataUrl: "data:image/png;base64,AAAA",
    });

    render(
      <MarkdownMessageContent
        content="![篇目库](docs/assets/cabin-library.png)"
        baseDir="/Users/a11111/code/wic"
        onCopyCode={vi.fn()}
        onOpenLink={vi.fn()}
      />,
    );

    const image = await screen.findByRole("img", { name: "篇目库" });
    expect(image).toHaveAttribute("src", "data:image/png;base64,AAAA");
    expect(localFilesMock.readImage).toHaveBeenCalledWith(
      "docs/assets/cabin-library.png",
      "/Users/a11111/code/wic",
    );
  });

  it("renders remote images directly and falls back to a placeholder on failure", async () => {
    localFilesMock.readImage.mockRejectedValue(new Error("File not found"));

    const { container } = render(
      <MarkdownMessageContent
        content={[
          "![remote](https://example.com/a.png)",
          "",
          "![missing](docs/missing.png)",
        ].join("\n")}
        baseDir="/tmp/session"
        onCopyCode={vi.fn()}
        onOpenLink={vi.fn()}
      />,
    );

    expect(await screen.findByRole("img", { name: "remote" })).toHaveAttribute(
      "src",
      "https://example.com/a.png",
    );
    expect(
      await screen.findByText("[Image failed to load: missing]"),
    ).toBeInTheDocument();
    expect(container.querySelectorAll("img")).toHaveLength(1);
  });

  it("explains missing local images and keeps the resolved path in the tooltip", async () => {
    localFilesMock.readImage.mockRejectedValue({
      code: "fileNotFound",
      message: "File not found: /tmp/session/docs/missing.png",
      path: "/tmp/session/docs/missing.png",
    });

    render(
      <MarkdownMessageContent
        content="![missing](docs/missing.png)"
        baseDir="/tmp/session"
        onCopyCode={vi.fn()}
        onOpenLink={vi.fn()}
      />,
    );

    const placeholder = await screen.findByText("[Image not found: missing]");
    expect(placeholder).toHaveAttribute(
      "title",
      expect.stringContaining("/tmp/session/docs/missing.png"),
    );
    expect(placeholder.getAttribute("title")).toContain("moved or deleted");
  });

  it("opens local images by their resolved absolute path", async () => {
    localFilesMock.readImage.mockResolvedValue({
      path: "/Users/a11111/code/wic/docs/assets/cabin-library.png",
      mime: "image/png",
      dataUrl: "data:image/png;base64,AAAA",
    });

    render(
      <MarkdownMessageContent
        content="![篇目库](docs/assets/cabin-library.png)"
        baseDir="/Users/a11111/code/wic"
        onCopyCode={vi.fn()}
        onOpenLink={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "篇目库" }));
    await waitFor(() =>
      expect(localFilesMock.openPath).toHaveBeenCalledWith(
        "/Users/a11111/code/wic/docs/assets/cabin-library.png",
        "/Users/a11111/code/wic",
      ),
    );
  });

  it("renders GFM tables and highlights cell content", () => {
    const { container } = render(
      <MarkdownMessageContent
        content={[
          "| Name | Value |",
          "| :--- | ---: |",
          "| **alpha** | 42 |",
          "| beta | 7 |",
        ].join("\n")}
        searchTerms={["alpha", "beta"]}
        onCopyCode={vi.fn()}
        onOpenLink={vi.fn()}
      />,
    );

    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(screen.getAllByRole("columnheader")).toHaveLength(2);
    expect(screen.getByRole("cell", { name: "42" })).toBeInTheDocument();
    expect(
      container.querySelector<HTMLElement>("table th")?.style.textAlign,
    ).toBe("left");
    expect(
      container.querySelector<HTMLElement>("table th + th")?.style.textAlign,
    ).toBe("right");
    expect(screen.getByText("alpha").tagName).toBe("MARK");
    expect(screen.getByText("beta").tagName).toBe("MARK");
  });

  it("renders fenced code cards and copies their source", () => {
    const onCopyCode = vi.fn();
    render(
      <MarkdownMessageContent
        content={"```ts\nconst value = 1;\n```"}
        onCopyCode={onCopyCode}
        onOpenLink={vi.fn()}
      />,
    );

    expect(screen.getByText("ts")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "复制代码" }));
    expect(onCopyCode).toHaveBeenCalledWith("const value = 1;");
  });

  it("escapes raw HTML and highlights rendered search matches", () => {
    const { container } = render(
      <MarkdownMessageContent
        content={"alpha <script>alert('x')</script> beta"}
        searchTerms={["alpha"]}
        onCopyCode={vi.fn()}
        onOpenLink={vi.fn()}
      />,
    );

    expect(container.querySelector("script")).not.toBeInTheDocument();
    expect(screen.getByText("alpha").tagName).toBe("MARK");
  });
});
