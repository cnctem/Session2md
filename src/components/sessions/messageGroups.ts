import type { SessionMessage } from "@/types";

export interface SessionMessageGroup {
  id: string;
  role: string;
  kind: "message" | "tool";
  content: string;
  reasoning: string;
  toolName?: string;
  toolCallId?: string;
  toolInput?: string;
  toolOutput?: string;
  ts?: number;
}

const appendSection = (current: string, next: string) => {
  const value = next.trim();
  if (!value) return current;
  return current ? `${current}\n\n${value}` : value;
};

const appendOutput = (current: string, next: string) => {
  const value = next.trim();
  if (!value) return current;
  return current ? `${current}\n${value}` : value;
};

const getMessageKind = (message: SessionMessage) => message.kind ?? "text";

export const groupSessionMessages = (
  messages: SessionMessage[],
): SessionMessageGroup[] => {
  const groups: SessionMessageGroup[] = [];
  const toolGroupsById = new Map<string, SessionMessageGroup>();
  let currentAssistant: SessionMessageGroup | null = null;
  let lastTool: SessionMessageGroup | null = null;

  const flushAssistant = () => {
    currentAssistant = null;
  };

  const createGroup = (
    message: SessionMessage,
    kind: SessionMessageGroup["kind"],
  ): SessionMessageGroup => {
    const group: SessionMessageGroup = {
      id: `message-${groups.length}`,
      role: kind === "tool" ? "tool" : message.role,
      kind,
      content: "",
      reasoning: "",
      ts: message.ts,
    };
    groups.push(group);
    return group;
  };

  for (const message of messages) {
    const kind = getMessageKind(message);
    const role = message.role.toLowerCase();

    if (kind === "reasoning" || kind === "text") {
      lastTool = null;
      if (role === "assistant") {
        if (!currentAssistant) {
          currentAssistant = createGroup(message, "message");
        }
        if (kind === "reasoning") {
          currentAssistant.reasoning = appendSection(
            currentAssistant.reasoning,
            message.content,
          );
        } else {
          currentAssistant.content = appendSection(
            currentAssistant.content,
            message.content,
          );
        }
        currentAssistant.ts ??= message.ts;
      } else {
        flushAssistant();
        const group = createGroup(message, "message");
        group.content = message.content.trim();
      }
      continue;
    }

    flushAssistant();
    if (kind === "toolCall") {
      const id = message.toolCallId;
      let group = id ? toolGroupsById.get(id) : undefined;
      if (!group) {
        group = createGroup(message, "tool");
        group.id = id ? `tool-${id}` : group.id;
        if (id) toolGroupsById.set(id, group);
      }
      group.toolName = message.toolName || group.toolName;
      group.toolCallId ||= id;
      group.toolInput = appendSection(group.toolInput ?? "", message.content);
      group.ts ??= message.ts;
      lastTool = group;
      continue;
    }

    const id = message.toolCallId;
    let group = id ? toolGroupsById.get(id) : undefined;
    if (!group && lastTool && !lastTool.toolOutput) {
      group = lastTool;
    }
    if (!group) {
      group = createGroup(message, "tool");
      group.id = id ? `tool-${id}` : group.id;
    }
    if (id) {
      toolGroupsById.set(id, group);
    }
    group.toolName = message.toolName || group.toolName;
    group.toolCallId ||= id;
    group.toolOutput = appendOutput(group.toolOutput ?? "", message.content);
    group.ts ??= message.ts;
    lastTool = group;
  }

  return groups;
};

export const getMessageGroupCopyText = (group: SessionMessageGroup) => {
  if (group.kind === "tool") {
    return [group.toolInput, group.toolOutput]
      .filter(Boolean)
      .join("\n\n---\n\n");
  }
  if (group.role.toLowerCase() !== "assistant") {
    return group.content;
  }
  return [group.reasoning, group.content].filter(Boolean).join("\n\n---\n\n");
};

const parseToolArguments = (value?: string): unknown => {
  if (!value) return null;
  try {
    return JSON.parse(value);
  } catch {
    return value;
  }
};

export const getToolInputDisplay = (group: SessionMessageGroup) => {
  const parsed = parseToolArguments(group.toolInput);
  const toolName = group.toolName?.toLowerCase() ?? "";
  const isShell = ["bash", "shell", "sh", "zsh"].includes(toolName);

  if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
    const object = parsed as Record<string, unknown>;
    if (isShell || "command" in object || "cmd" in object) {
      const command = object.command ?? object.cmd;
      if (typeof command === "string") {
        return { language: "bash", text: command };
      }
      if (Array.isArray(command)) {
        return { language: "bash", text: command.join(" ") };
      }
    }
    return { language: "json", text: JSON.stringify(parsed, null, 2) };
  }

  if (typeof parsed === "string") {
    return { language: isShell ? "bash" : "text", text: parsed };
  }
  if (parsed !== null) {
    return { language: "json", text: JSON.stringify(parsed, null, 2) };
  }
  return { language: "text", text: group.toolInput ?? "" };
};
