import { http, HttpResponse } from "msw";
import {
  deleteSession,
  getSession2mdSettings,
  getSessionMessages,
  listSessions,
  saveSession2mdSettings,
} from "./state";

const TAURI_ENDPOINT = "http://tauri.local";

const withJson = async <T>(request: Request): Promise<T> => {
  try {
    const body = await request.text();
    if (!body) return {} as T;
    return JSON.parse(body) as T;
  } catch {
    return {} as T;
  }
};

const success = <T>(payload: T) => HttpResponse.json(payload as any);

export const handlers = [
  http.post(`${TAURI_ENDPOINT}/list_sessions`, () => success(listSessions())),

  http.post(`${TAURI_ENDPOINT}/get_session2md_settings`, () =>
    success(getSession2mdSettings()),
  ),

  http.post(
    `${TAURI_ENDPOINT}/save_session2md_settings`,
    async ({ request }) => {
      const { settings } = await withJson<{
        settings: Parameters<typeof saveSession2mdSettings>[0];
      }>(request);
      return success(saveSession2mdSettings(settings));
    },
  ),

  http.post(`${TAURI_ENDPOINT}/get_session_messages`, async ({ request }) => {
    const { providerId, sourcePath } = await withJson<{
      providerId: string;
      sourcePath: string;
    }>(request);
    return success(getSessionMessages(providerId, sourcePath));
  }),

  http.post(`${TAURI_ENDPOINT}/open_external_url`, () => success(true)),

  http.post(`${TAURI_ENDPOINT}/delete_session`, async ({ request }) => {
    const { providerId, sessionId, sourcePath } = await withJson<{
      providerId: string;
      sessionId: string;
      sourcePath: string;
    }>(request);
    return success(deleteSession(providerId, sessionId, sourcePath));
  }),

  http.post(`${TAURI_ENDPOINT}/delete_sessions`, async ({ request }) => {
    const { items = [] } = await withJson<{
      items?: { providerId: string; sessionId: string; sourcePath: string }[];
    }>(request);
    return success(
      items.map((item) => ({
        ...item,
        success: deleteSession(
          item.providerId,
          item.sessionId,
          item.sourcePath,
        ),
      })),
    );
  }),
];
