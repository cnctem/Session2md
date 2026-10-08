import type {
  Session2mdSettings,
  Session2mdSettingsSnapshot,
} from "@/lib/api/session2mdSettings";
import {
  SESSION_DIRECTORY_IDS,
  type SessionProviderId,
} from "@/lib/sessionProviders";
import type {
  SessionMessage,
  SessionMeta,
  SessionSearchHit,
  SessionSearchRequest,
  SessionSearchResponse,
} from "@/types";

const clone = <T>(value: T): T => structuredClone(value);

const sessionMessageKey = (providerId: string, sourcePath: string) =>
  `${providerId}:${sourcePath}`;

const createDefaultSettings = (): Session2mdSettings => ({
  directoryOverrides: {},
  hiddenProviders: [],
  autoCheckUpdates: null,
  promptBeforeExport: true,
  exportThinking: false,
  exportToolInputs: false,
  exportToolOutputs: false,
  exportSystem: false,
  defaultExpandThinking: false,
  defaultExpandTools: false,
  defaultExpandSystem: false,
  renderMarkdown: true,
});

let sessionsState: SessionMeta[] = [];
let sessionMessagesState: Record<string, SessionMessage[]> = {};
let session2mdSettingsState = createDefaultSettings();

export const resetProviderState = () => {
  sessionsState = [];
  sessionMessagesState = {};
  session2mdSettingsState = createDefaultSettings();
};

export const listSessions = () => clone(sessionsState);

export const getSessionMessages = (providerId: string, sourcePath: string) =>
  clone(sessionMessagesState[sessionMessageKey(providerId, sourcePath)] ?? []);

export const searchSessions = (
  request: SessionSearchRequest,
): SessionSearchResponse => {
  const terms = request.query.toLowerCase().split(/\s+/).filter(Boolean);
  const results = sessionsState.flatMap((session) => {
    if (
      request.providerIds.length > 0 &&
      !request.providerIds.includes(session.providerId)
    ) {
      return [];
    }
    if (
      request.projectDir &&
      !session.projectDir?.toLowerCase().includes(request.projectDir.toLowerCase())
    ) {
      return [];
    }
    const activeAt = session.lastActiveAt ?? session.createdAt;
    if (request.activeFrom !== undefined || request.activeTo !== undefined) {
      if (
        activeAt === undefined ||
        (request.activeFrom !== undefined && activeAt < request.activeFrom) ||
        (request.activeTo !== undefined && activeAt > request.activeTo)
      ) {
        return [];
      }
    }

    const metadata = [
      { field: "title" as const, text: session.title },
      { field: "summary" as const, text: session.summary },
      { field: "sessionId" as const, text: session.sessionId },
      { field: "projectDir" as const, text: session.projectDir ?? undefined },
      { field: "sourcePath" as const, text: session.sourcePath },
    ];
    const messages = getSessionMessages(
      session.providerId,
      session.sourcePath ?? "",
    );
    const hits: SessionSearchHit[] = [];

    if (terms.length > 0) {
      for (const item of metadata) {
        const text = item.text?.trim();
        if (
          text &&
          terms.every((term) => text.toLowerCase().includes(term))
        ) {
          hits.push({ field: item.field, snippet: text });
        }
      }
      messages.forEach((message, messageIndex) => {
        const role = message.role.toLowerCase();
        const kind = message.kind ?? "text";
        if (
          request.roles.length > 0 &&
          !request.roles.includes(role)
        ) {
          return;
        }
        if (
          request.messageKinds.length > 0 &&
          !request.messageKinds.includes(kind)
        ) {
          return;
        }
        if (
          terms.every((term) => message.content.toLowerCase().includes(term))
        ) {
          hits.push({
            field: "message",
            messageIndex,
            role: message.role,
            kind,
            ts: message.ts,
            snippet: message.content,
          });
        }
      });
    } else if (
      request.roles.length > 0 ||
      request.messageKinds.length > 0
    ) {
      messages.forEach((message, messageIndex) => {
        const role = message.role.toLowerCase();
        const kind = message.kind ?? "text";
        if (request.roles.length > 0 && !request.roles.includes(role)) {
          return;
        }
        if (
          request.messageKinds.length > 0 &&
          !request.messageKinds.includes(kind)
        ) {
          return;
        }
        hits.push({
          field: "message",
          messageIndex,
          role: message.role,
          kind,
          ts: message.ts,
          snippet: message.content,
        });
      });
    } else {
      return [
        {
          session: clone(session),
          score: 0,
          totalMatches: 0,
          hits: [],
        },
      ];
    }

    if (hits.length === 0) return [];
    return [
      {
        session: clone(session),
        score: hits.length,
        totalMatches: hits.length,
        hits: hits.slice(0, 20),
      },
    ];
  });

  return {
    results: results.slice(0, 100),
    totalSessions: results.length,
    totalMatches: results.reduce(
      (total, result) => total + result.totalMatches,
      0,
    ),
    truncated: results.length > 100,
    failedSessions: 0,
  };
};

export const deleteSession = (
  providerId: string,
  sessionId: string,
  sourcePath: string,
) => {
  const index = sessionsState.findIndex(
    (session) =>
      session.providerId === providerId &&
      session.sessionId === sessionId &&
      session.sourcePath === sourcePath,
  );
  if (index < 0) return false;

  sessionsState.splice(index, 1);
  delete sessionMessagesState[sessionMessageKey(providerId, sourcePath)];
  return true;
};

export const getSession2mdSettings = (): Session2mdSettingsSnapshot => ({
  ...clone(session2mdSettingsState),
  resolvedDirectories: Object.fromEntries(
    SESSION_DIRECTORY_IDS.map((directoryId) => [
      directoryId,
      `/mock/${directoryId}`,
    ]),
  ) as Session2mdSettingsSnapshot["resolvedDirectories"],
});

export const saveSession2mdSettings = (settings: Session2mdSettings) => {
  session2mdSettingsState = clone(settings);
  return getSession2mdSettings();
};

export const setHiddenSessionProviders = (providerIds: SessionProviderId[]) => {
  session2mdSettingsState = {
    ...session2mdSettingsState,
    hiddenProviders: [...providerIds],
  };
};

export const setSession2mdRenderMarkdown = (renderMarkdown: boolean) => {
  session2mdSettingsState = {
    ...session2mdSettingsState,
    renderMarkdown,
  };
};

export const setSession2mdPromptBeforeExport = (
  promptBeforeExport: boolean,
) => {
  session2mdSettingsState = {
    ...session2mdSettingsState,
    promptBeforeExport,
  };
};

export const setSession2mdExportOptions = (options: {
  exportThinking?: boolean;
  exportToolInputs?: boolean;
  exportToolOutputs?: boolean;
  exportSystem?: boolean;
}) => {
  session2mdSettingsState = {
    ...session2mdSettingsState,
    ...options,
  };
};

export const setSession2mdDefaultExpansion = (settings: {
  defaultExpandThinking?: boolean;
  defaultExpandTools?: boolean;
  defaultExpandSystem?: boolean;
}) => {
  session2mdSettingsState = {
    ...session2mdSettingsState,
    ...settings,
  };
};

export const setSessionFixtures = (
  sessions: SessionMeta[],
  messages: Record<string, SessionMessage[]>,
) => {
  sessionsState = clone(sessions);
  sessionMessagesState = clone(messages);
};
