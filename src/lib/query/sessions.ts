import { useQuery } from "@tanstack/react-query";
import { sessionsApi } from "@/lib/api/sessions";
import type { SessionMessage, SessionMeta } from "@/types";

export const useSessionsQuery = () =>
  useQuery<SessionMeta[]>({
    queryKey: ["sessions"],
    queryFn: () => sessionsApi.list(),
    staleTime: 30 * 1000,
  });

export const useSessionMessagesQuery = (
  providerId?: string,
  sourcePath?: string,
) =>
  useQuery<SessionMessage[]>({
    queryKey: ["sessionMessages", providerId, sourcePath],
    queryFn: () => sessionsApi.getMessages(providerId!, sourcePath!),
    enabled: Boolean(providerId && sourcePath),
    staleTime: 30 * 1000,
  });
