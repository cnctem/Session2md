import { useQuery } from "@tanstack/react-query";
import { session2mdSettingsApi } from "@/lib/api/session2mdSettings";

export const session2mdSettingsKey = ["session2mdSettings"] as const;

export const useSession2mdSettingsQuery = () =>
  useQuery({
    queryKey: session2mdSettingsKey,
    queryFn: session2mdSettingsApi.get,
  });
