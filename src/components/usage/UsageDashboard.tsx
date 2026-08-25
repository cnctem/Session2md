import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useQueryClient } from "@tanstack/react-query";
import { motion } from "framer-motion";
import {
  Activity,
  BarChart3,
  LayoutGrid,
  ListFilter,
  RefreshCw,
} from "lucide-react";
import { UsageHero } from "./UsageHero";
import { UsageTrendChart } from "./UsageTrendChart";
import { RequestLogTable } from "./RequestLogTable";
import { ProviderStatsTable } from "./ProviderStatsTable";
import { ModelStatsTable } from "./ModelStatsTable";
import {
  KNOWN_APP_TYPES,
  type AppType,
  type AppTypeFilter,
  type UsageRangeSelection,
} from "@/types/usage";
import { ProviderIcon } from "@/components/ProviderIcon";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";
import { usageKeys, useModelStats, useProviderStats } from "@/lib/query/usage";
import { getLocaleFromLanguage } from "./format";
import { getUsageRangePresetLabel, resolveUsageRange } from "@/lib/usageRange";
import { UsageDateRangePicker } from "./UsageDateRangePicker";

const APP_FILTER_OPTIONS: AppTypeFilter[] = ["all", ...KNOWN_APP_TYPES];
const DEFAULT_REFRESH_INTERVAL_MS = 30000;
const REFRESH_INTERVAL_OPTIONS_MS = [0, 5000, 10000, 30000, 60000] as const;
type RefreshIntervalOption = (typeof REFRESH_INTERVAL_OPTIONS_MS)[number];

const APP_FILTER_ICON: Record<AppType, string> = {
  claude: "claude",
  codex: "openai",
  gemini: "gemini",
  grokbuild: "grok",
  opencode: "opencode",
  pi: "pi",
};

const DYNAMIC_OPTION_PREFIX = "v:";
const encodeOptionValue = (name: string) => `${DYNAMIC_OPTION_PREFIX}${name}`;
const decodeOptionValue = (value: string) =>
  value === "all" ? undefined : value.slice(DYNAMIC_OPTION_PREFIX.length);

interface UsageDashboardProps {
  refreshIntervalMs?: number;
  onRefreshIntervalChange?: (next: number) => Promise<boolean> | boolean | void;
}

const isRefreshIntervalOption = (
  value: number | undefined,
): value is RefreshIntervalOption =>
  REFRESH_INTERVAL_OPTIONS_MS.includes(value as RefreshIntervalOption);

const normalizeRefreshInterval = (value: number | undefined) =>
  isRefreshIntervalOption(value) ? value : DEFAULT_REFRESH_INTERVAL_MS;

export function UsageDashboard(props: UsageDashboardProps = {}) {
  return <UsageDashboardContent {...props} />;
}

function UsageDashboardContent({
  refreshIntervalMs: savedRefreshIntervalMs,
  onRefreshIntervalChange,
}: UsageDashboardProps) {
  const { t, i18n } = useTranslation();
  const queryClient = useQueryClient();
  const [range, setRange] = useState<UsageRangeSelection>({ preset: "today" });
  const [appType, setAppType] = useState<AppTypeFilter>("all");
  const [providerName, setProviderName] = useState<string | undefined>();
  const [model, setModel] = useState<string | undefined>();
  const [refreshIntervalMs, setRefreshIntervalMs] = useState(() =>
    normalizeRefreshInterval(savedRefreshIntervalMs),
  );

  useEffect(() => {
    setRefreshIntervalMs(normalizeRefreshInterval(savedRefreshIntervalMs));
  }, [savedRefreshIntervalMs]);

  const changeAppType = (next: AppTypeFilter) => {
    setAppType(next);
    if (next !== appType) {
      setProviderName(undefined);
      setModel(undefined);
    }
  };

  const changeProviderName = (next: string | undefined) => {
    setProviderName(next);
    if (next !== providerName) setModel(undefined);
  };

  const changeRefreshInterval = async (next: number) => {
    const normalized = normalizeRefreshInterval(next);
    const previous = refreshIntervalMs;
    setRefreshIntervalMs(normalized);
    await queryClient.invalidateQueries({ queryKey: usageKeys.all });
    try {
      const saved = await onRefreshIntervalChange?.(normalized);
      if (saved === false) setRefreshIntervalMs(previous);
    } catch (error) {
      console.error(
        "[UsageDashboard] Failed to persist refresh interval",
        error,
      );
      setRefreshIntervalMs(previous);
    }
  };

  const language = i18n.resolvedLanguage || i18n.language || "en";
  const locale = getLocaleFromLanguage(language);
  const resolvedRange = useMemo(() => resolveUsageRange(range), [range]);
  const rangeLabel = useMemo(() => {
    if (range.preset !== "custom")
      return getUsageRangePresetLabel(range.preset, t);
    const start = new Date(resolvedRange.startDate * 1000).toLocaleString(
      locale,
    );
    if (range.liveEndTime) return `${start} -> ${t("usage.liveEndTimeNow")}`;
    return `${start} - ${new Date(resolvedRange.endDate * 1000).toLocaleString(locale)}`;
  }, [locale, range, resolvedRange.endDate, resolvedRange.startDate, t]);

  const optionsRefetch = {
    refetchInterval:
      refreshIntervalMs > 0 ? refreshIntervalMs : (false as const),
  };
  const { data: providerOptionsData } = useProviderStats(
    range,
    { appType },
    optionsRefetch,
  );
  const { data: modelOptionsData } = useModelStats(
    range,
    { appType, providerName },
    optionsRefetch,
  );
  const providerOptions = useMemo(() => {
    const values = new Set(
      (providerOptionsData ?? []).map((item) => item.providerName),
    );
    if (providerName) values.add(providerName);
    return Array.from(values);
  }, [providerName, providerOptionsData]);
  const modelOptions = useMemo(() => {
    const values = new Set((modelOptionsData ?? []).map((item) => item.model));
    if (model) values.add(model);
    return Array.from(values);
  }, [model, modelOptionsData]);

  return (
    <motion.div
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.25 }}
      className="space-y-8 pb-8"
    >
      <div className="flex flex-col justify-between gap-4 lg:flex-row lg:items-end">
        <div className="space-y-1">
          <h2 className="text-xl font-semibold">{t("usage.title")}</h2>
          <p className="text-sm text-muted-foreground">{t("usage.subtitle")}</p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <div className="flex items-center gap-1 rounded-md border border-border/50 bg-muted/30 p-1">
            {APP_FILTER_OPTIONS.map((type) => {
              const label = t(`usage.appFilter.${type}`);
              return (
                <button
                  key={type}
                  type="button"
                  title={label}
                  aria-label={label}
                  onClick={() => changeAppType(type)}
                  className={cn(
                    "flex size-8 items-center justify-center rounded-md transition-colors",
                    appType === type
                      ? "bg-background text-primary shadow-sm"
                      : "text-muted-foreground hover:bg-muted hover:text-foreground",
                  )}
                >
                  {type === "all" ? (
                    <LayoutGrid className="size-4" />
                  ) : (
                    <ProviderIcon
                      icon={APP_FILTER_ICON[type]}
                      name={label}
                      size={16}
                    />
                  )}
                </button>
              );
            })}
          </div>
          <Select
            value={providerName ? encodeOptionValue(providerName) : "all"}
            onValueChange={(value) =>
              changeProviderName(decodeOptionValue(value))
            }
          >
            <SelectTrigger className="h-9 w-28 text-xs" title={providerName}>
              <SelectValue placeholder={t("usage.filterBySource")} />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">{t("usage.allSources")}</SelectItem>
              {providerOptions.map((name) => (
                <SelectItem key={name} value={encodeOptionValue(name)}>
                  {name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select
            value={model ? encodeOptionValue(model) : "all"}
            onValueChange={(value) => setModel(decodeOptionValue(value))}
          >
            <SelectTrigger className="h-9 w-28 text-xs" title={model}>
              <SelectValue placeholder={t("usage.filterByModel")} />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">{t("usage.allModels")}</SelectItem>
              {modelOptions.map((name) => (
                <SelectItem key={name} value={encodeOptionValue(name)}>
                  {name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select
            value={String(refreshIntervalMs)}
            onValueChange={(value) => void changeRefreshInterval(Number(value))}
          >
            <SelectTrigger
              className="h-9 w-28 text-xs"
              aria-label={t("usage.refreshInterval")}
            >
              <span className="flex items-center gap-2">
                <RefreshCw className="size-3.5" />
                <SelectValue />
              </span>
            </SelectTrigger>
            <SelectContent>
              {REFRESH_INTERVAL_OPTIONS_MS.map((ms) => (
                <SelectItem key={ms} value={String(ms)}>
                  {ms > 0 ? `${ms / 1000}s` : t("usage.refreshOff")}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <UsageDateRangePicker
            selection={range}
            triggerLabel={rangeLabel}
            onApply={setRange}
          />
        </div>
      </div>

      <UsageHero
        range={range}
        appType={appType === "all" ? undefined : appType}
        providerName={providerName}
        model={model}
        refreshIntervalMs={refreshIntervalMs}
      />
      <UsageTrendChart
        range={range}
        rangeLabel={rangeLabel}
        appType={appType}
        providerName={providerName}
        model={model}
        refreshIntervalMs={refreshIntervalMs}
      />

      <Tabs defaultValue="logs" className="w-full">
        <TabsList className="mb-4">
          <TabsTrigger value="logs" className="gap-2">
            <ListFilter className="size-4" />
            {t("usage.requestLogs")}
          </TabsTrigger>
          <TabsTrigger value="providers" className="gap-2">
            <Activity className="size-4" />
            {t("usage.providerStats")}
          </TabsTrigger>
          <TabsTrigger value="models" className="gap-2">
            <BarChart3 className="size-4" />
            {t("usage.modelStats")}
          </TabsTrigger>
        </TabsList>
        <TabsContent value="logs" className="mt-0">
          <RequestLogTable
            range={range}
            rangeLabel={rangeLabel}
            appType={appType}
            providerName={providerName}
            model={model}
            refreshIntervalMs={refreshIntervalMs}
            onRangeChange={setRange}
          />
        </TabsContent>
        <TabsContent value="providers" className="mt-0">
          <ProviderStatsTable
            range={range}
            appType={appType}
            providerName={providerName}
            model={model}
            refreshIntervalMs={refreshIntervalMs}
          />
        </TabsContent>
        <TabsContent value="models" className="mt-0">
          <ModelStatsTable
            range={range}
            appType={appType}
            providerName={providerName}
            model={model}
            refreshIntervalMs={refreshIntervalMs}
          />
        </TabsContent>
      </Tabs>
    </motion.div>
  );
}
