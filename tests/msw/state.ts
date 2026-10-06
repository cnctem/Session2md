import type {
  Session2mdSettings,
  Session2mdSettingsSnapshot,
} from "@/lib/api/session2mdSettings";
import {
  SESSION_DIRECTORY_IDS,
  type SessionProviderId,
} from "@/lib/sessionProviders";
import type { SessionMessage, SessionMeta } from "@/types";

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
