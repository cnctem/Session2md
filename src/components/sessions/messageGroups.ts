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

type MessageGroupBuffers = {
  content: string[];
  reasoning: string[];
  toolInput: string[];
  toolOutput: string[];
};

const getMessageKind = (message: SessionMessage) => message.kind ?? "text";

export const groupSessionMessages = (
  messages: SessionMessage[],
): SessionMessageGroup[] => {
  const groups: SessionMessageGroup[] = [];
  const groupBuffers = new WeakMap<SessionMessageGroup, MessageGroupBuffers>();
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
    groupBuffers.set(group, {
      content: [],
      reasoning: [],
      toolInput: [],
      toolOutput: [],
    });
    return group;
  };

  const append = (
    group: SessionMessageGroup,
    key: keyof MessageGroupBuffers,
    value: string,
  ) => {
    const trimmed = value.trim();
    if (trimmed) groupBuffers.get(group)?.[key].push(trimmed);
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
          append(currentAssistant, "reasoning", message.content);
        } else {
          append(currentAssistant, "content", message.content);
        }
        currentAssistant.ts ??= message.ts;
      } else {
        flushAssistant();
        const group = createGroup(message, "message");
        append(group, "content", message.content);
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
      append(group, "toolInput", message.content);
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
    append(group, "toolOutput", message.content);
    group.ts ??= message.ts;
    lastTool = group;
  }

  for (const group of groups) {
    const buffers = groupBuffers.get(group);
    if (!buffers) continue;
    group.content = buffers.content.join("\n\n");
    group.reasoning = buffers.reasoning.join("\n\n");
    group.toolInput = buffers.toolInput.length
      ? buffers.toolInput.join("\n\n")
      : undefined;
    group.toolOutput = buffers.toolOutput.length
      ? buffers.toolOutput.join("\n")
      : undefined;
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

export interface ToolInputDisplay {
  language: "bash" | "json" | "text";
  text: string;
}

const toolInputDisplayCache = new WeakMap<
  SessionMessageGroup,
  ToolInputDisplay
>();

export const getToolInputDisplay = (
  group: SessionMessageGroup,
): ToolInputDisplay => {
  const cached = toolInputDisplayCache.get(group);
  if (cached) return cached;

  const parsed = parseToolArguments(group.toolInput);
  const toolName = group.toolName?.toLowerCase() ?? "";
  const isShell = ["bash", "shell", "sh", "zsh"].includes(toolName);

  let display: ToolInputDisplay;
  if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
    const object = parsed as Record<string, unknown>;
    if (isShell || "command" in object || "cmd" in object) {
      const command = object.command ?? object.cmd;
      if (typeof command === "string") {
        display = { language: "bash", text: command };
      } else if (Array.isArray(command)) {
        display = { language: "bash", text: command.join(" ") };
      } else {
        display = { language: "json", text: JSON.stringify(parsed, null, 2) };
      }
    } else {
      display = { language: "json", text: JSON.stringify(parsed, null, 2) };
    }
  } else if (typeof parsed === "string") {
    display = { language: isShell ? "bash" : "text", text: parsed };
  } else if (parsed !== null) {
    display = { language: "json", text: JSON.stringify(parsed, null, 2) };
  } else {
    display = { language: "text", text: group.toolInput ?? "" };
  }

  toolInputDisplayCache.set(group, display);
  return display;
};
