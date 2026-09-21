import { describe, expect, it } from "vitest";
import {
  getMessageGroupCopyText,
  groupSessionMessages,
} from "@/components/sessions/messageGroups";
import type { SessionMessage } from "@/types";

describe("message groups", () => {
  it("groups thinking and assistant text into one bubble", () => {
    const messages: SessionMessage[] = [
      { role: "assistant", content: "thinking", kind: "reasoning", ts: 1 },
      { role: "assistant", content: "answer", kind: "text", ts: 2 },
    ];

    const groups = groupSessionMessages(messages);
    expect(groups).toHaveLength(1);
    expect(groups[0]).toMatchObject({
      role: "assistant",
      kind: "message",
      reasoning: "thinking",
      content: "answer",
    });
    expect(getMessageGroupCopyText(groups[0])).toBe(
      "thinking\n\n---\n\nanswer",
    );
  });

  it("pairs tool calls and results by call id", () => {
    const messages: SessionMessage[] = [
      {
        role: "tool",
        content: '{"command":"pwd"}',
        kind: "toolCall",
        toolCallId: "call-1",
        toolName: "bash",
      },
      {
        role: "tool",
        content: "/tmp/project",
        kind: "toolResult",
        toolCallId: "call-1",
        toolName: "bash",
      },
    ];

    const groups = groupSessionMessages(messages);
    expect(groups).toHaveLength(1);
    expect(groups[0]).toMatchObject({
      kind: "tool",
      toolCallId: "call-1",
      toolName: "bash",
      toolInput: '{"command":"pwd"}',
      toolOutput: "/tmp/project",
    });
  });

  it("keeps orphan tool results instead of dropping them", () => {
    const groups = groupSessionMessages([
      { role: "tool", content: "orphan output", kind: "toolResult" },
    ]);

    expect(groups).toHaveLength(1);
    expect(groups[0].toolOutput).toBe("orphan output");
  });
});
