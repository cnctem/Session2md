import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MarkdownMessageContent } from "@/components/sessions/MarkdownMessageContent";

describe("MarkdownMessageContent", () => {
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
        searchQuery="alpha"
        onCopyCode={vi.fn()}
        onOpenLink={vi.fn()}
      />,
    );

    expect(container.querySelector("script")).not.toBeInTheDocument();
    expect(screen.getByText("alpha").tagName).toBe("MARK");
  });
});
