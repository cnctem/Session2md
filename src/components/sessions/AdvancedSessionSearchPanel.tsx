import { forwardRef, useImperativeHandle, useMemo, useState } from "react";
import { ChevronDown, RotateCw, SlidersHorizontal } from "lucide-react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { getProviderLabel } from "./utils";
import type {
  SessionMessageKind,
  SessionSearchProgress,
  SessionSearchRequest,
} from "@/types";
import { generateUUID } from "@/utils/uuid";

const DEFAULT_ROLES = ["user", "assistant"];
const DEFAULT_MESSAGE_KINDS: SessionMessageKind[] = ["text"];

interface MultiSelectOption {
  value: string;
  label: string;
}

interface MultiSelectProps {
  label: string;
  allLabel: string;
  options: MultiSelectOption[];
  selected: string[];
  onChange: (selected: string[]) => void;
}

function MultiSelect({
  label,
  allLabel,
  options,
  selected,
  onChange,
}: MultiSelectProps) {
  const selectedSet = new Set(selected);
  const summary = selected.length
    ? selected
        .map(
          (value) =>
            options.find((option) => option.value === value)?.label ?? value,
        )
        .join(", ")
    : allLabel;

  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button
          type="button"
          variant="outline"
          className="h-8 w-full justify-between gap-2 px-2 text-xs font-normal text-foreground hover:text-foreground"
        >
          <span className="min-w-0 truncate text-left">{summary}</span>
          <ChevronDown className="size-3.5 shrink-0 opacity-60" />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-60 p-2">
        <p className="mb-1 px-1 text-[11px] font-medium text-muted-foreground">
          {label}
        </p>
        <div className="grid gap-1">
          {options.map((option) => (
            <label
              key={option.value}
              className="flex cursor-pointer items-center gap-2 rounded-sm px-1.5 py-1 text-xs hover:bg-muted"
            >
              <Checkbox
                checked={selectedSet.has(option.value)}
                onCheckedChange={(checked) => {
                  const next = new Set(selectedSet);
                  if (checked) next.add(option.value);
                  else next.delete(option.value);
                  onChange(Array.from(next));
                }}
              />
              <span>{option.label}</span>
            </label>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  );
}

interface AdvancedSessionSearchPanelProps {
  open: boolean;
  query: string;
  providerOptions: string[];
  isSearching: boolean;
  progress?: SessionSearchProgress;
  error?: string;
  hasActiveSearch: boolean;
  onSearch: (request: SessionSearchRequest) => void;
  onClear: () => void;
  onRefresh: () => void;
}

export interface AdvancedSessionSearchHandle {
  submit: () => boolean;
  reset: () => void;
}

const toStartOfLocalDay = (value: string) => {
  if (!value) return undefined;
  const timestamp = new Date(`${value}T00:00:00`).getTime();
  return Number.isFinite(timestamp) ? timestamp : undefined;
};

const toEndOfLocalDay = (value: string) => {
  if (!value) return undefined;
  const timestamp = new Date(`${value}T23:59:59.999`).getTime();
  return Number.isFinite(timestamp) ? timestamp : undefined;
};

const toDateInputValue = (date: Date) =>
  `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;

const getDefaultActiveDateRange = () => {
  const today = new Date();
  const sevenDaysAgo = new Date(today);
  sevenDaysAgo.setDate(today.getDate() - 7);
  return {
    from: toDateInputValue(sevenDaysAgo),
    to: toDateInputValue(today),
  };
};

export const AdvancedSessionSearchPanel = forwardRef<
  AdvancedSessionSearchHandle,
  AdvancedSessionSearchPanelProps
>(function AdvancedSessionSearchPanel(
  {
    open,
    query,
    providerOptions,
    isSearching,
    progress,
    error,
    hasActiveSearch,
    onSearch,
    onClear,
    onRefresh,
  },
  ref,
) {
  const { t } = useTranslation();
  const [defaultDateRange] = useState(getDefaultActiveDateRange);
  const [projectDir, setProjectDir] = useState("");
  const [activeFrom, setActiveFrom] = useState(defaultDateRange.from);
  const [activeTo, setActiveTo] = useState(defaultDateRange.to);
  const [providers, setProviders] = useState<string[]>([]);
  const [roles, setRoles] = useState<string[]>(DEFAULT_ROLES);
  const [messageKinds, setMessageKinds] = useState<SessionMessageKind[]>(
    DEFAULT_MESSAGE_KINDS,
  );
  const [validationError, setValidationError] = useState<string | null>(null);

  const providerSelectOptions = useMemo(
    () =>
      providerOptions.map((providerId) => ({
        value: providerId,
        label: getProviderLabel(providerId, t),
      })),
    [providerOptions, t],
  );

  const roleOptions = useMemo<MultiSelectOption[]>(
    () => [
      { value: "user", label: t("sessionManager.roleUser") },
      { value: "assistant", label: "AI" },
      { value: "system", label: t("sessionManager.roleSystem") },
      { value: "tool", label: t("sessionManager.roleTool") },
    ],
    [t],
  );

  const messageKindOptions = useMemo<MultiSelectOption[]>(
    () => [
      { value: "text", label: t("sessionManager.advanced.kindText") },
      { value: "reasoning", label: t("sessionManager.thinking") },
      { value: "toolCall", label: t("sessionManager.toolCall") },
      { value: "toolResult", label: t("sessionManager.toolResult") },
    ],
    [t],
  );

  const resetForm = () => {
    setProjectDir("");
    setActiveFrom(defaultDateRange.from);
    setActiveTo(defaultDateRange.to);
    setProviders([]);
    setRoles(DEFAULT_ROLES);
    setMessageKinds(DEFAULT_MESSAGE_KINDS);
    setValidationError(null);
    onClear();
  };

  const submit = () => {
    const queryValue = query.trim();
    const projectDirValue = projectDir.trim();
    const from = toStartOfLocalDay(activeFrom);
    const to = toEndOfLocalDay(activeTo);
    const hasCustomDateRange =
      (Boolean(activeFrom) && activeFrom !== defaultDateRange.from) ||
      (Boolean(activeTo) && activeTo !== defaultDateRange.to);
    const hasCustomMessageScope =
      roles.length !== DEFAULT_ROLES.length ||
      roles.some((role) => !DEFAULT_ROLES.includes(role)) ||
      messageKinds.length !== DEFAULT_MESSAGE_KINDS.length ||
      messageKinds.some((kind) => !DEFAULT_MESSAGE_KINDS.includes(kind));
    const hasCondition =
      Boolean(queryValue) ||
      Boolean(projectDirValue) ||
      hasCustomDateRange ||
      providers.length > 0 ||
      hasCustomMessageScope;

    if (!hasCondition) {
      setValidationError(t("sessionManager.advanced.noConditions"));
      return false;
    }
    if (from !== undefined && to !== undefined && from > to) {
      setValidationError(t("sessionManager.advanced.invalidDateRange"));
      return false;
    }
    if (roles.length === 0 || messageKinds.length === 0) {
      setValidationError(t("sessionManager.advanced.emptyMessageScope"));
      return false;
    }

    setValidationError(null);
    const useMessageScope = Boolean(queryValue) || hasCustomMessageScope;
    onSearch({
      requestId: generateUUID(),
      query: queryValue,
      providerIds: providers,
      projectDir: projectDirValue || undefined,
      activeFrom: from,
      activeTo: to,
      roles: useMessageScope ? roles : [],
      messageKinds: useMessageScope ? messageKinds : [],
      forceRefresh: false,
    });
    return true;
  };

  useImperativeHandle(ref, () => ({
    submit,
    reset: resetForm,
  }));

  const progressLabel = (() => {
    if (!isSearching) return null;
    if (progress?.phase === "indexing") {
      return t("sessionManager.advanced.indexingProgress", {
        indexed: progress.indexed,
        total: progress.total,
      });
    }
    if (progress?.phase === "searching") {
      return t("sessionManager.advanced.searchingProgress");
    }
    return t("sessionManager.advanced.scanningProgress");
  })();

  return (
    <form
      className={
        open
          ? "grid max-h-[55vh] gap-3 overflow-y-auto overscroll-contain rounded-md border bg-muted/25 p-3 pr-2"
          : "hidden"
      }
      onSubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-1.5 text-xs font-medium">
          <SlidersHorizontal className="size-3.5" />
          {t("sessionManager.advanced.title")}
        </div>
        <div className="flex items-center gap-1">
          {hasActiveSearch && (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-6 gap-1 px-1.5 text-[11px]"
              onClick={onRefresh}
              disabled={isSearching}
            >
              <RotateCw className="size-3" />
              {t("sessionManager.advanced.refreshIndex")}
            </Button>
          )}
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="h-6 px-1.5 text-[11px]"
            onClick={resetForm}
            disabled={isSearching}
          >
            {t("sessionManager.advanced.clear")}
          </Button>
        </div>
      </div>

      <div className="grid gap-1.5">
        <Label className="text-xs">
          {t("sessionManager.advanced.providers")}
        </Label>
        <MultiSelect
          label={t("sessionManager.advanced.providers")}
          allLabel={t("sessionManager.providerFilterAll")}
          options={providerSelectOptions}
          selected={providers}
          onChange={setProviders}
        />
      </div>

      <div className="grid gap-1.5">
        <Label htmlFor="advanced-session-project" className="text-xs">
          {t("sessionManager.advanced.projectDir")}
        </Label>
        <Input
          id="advanced-session-project"
          value={projectDir}
          onChange={(event) => setProjectDir(event.target.value)}
          placeholder={t("sessionManager.advanced.projectDirPlaceholder")}
          className="h-8 text-xs"
        />
      </div>

      <div className="grid grid-cols-2 gap-2">
        <div className="grid gap-1.5">
          <Label htmlFor="advanced-session-from" className="text-xs">
            {t("sessionManager.advanced.activeFrom")}
          </Label>
          <Input
            id="advanced-session-from"
            type="date"
            value={activeFrom}
            max={activeTo || undefined}
            onChange={(event) => setActiveFrom(event.target.value)}
            className="h-8 text-xs"
          />
        </div>
        <div className="grid gap-1.5">
          <Label htmlFor="advanced-session-to" className="text-xs">
            {t("sessionManager.advanced.activeTo")}
          </Label>
          <Input
            id="advanced-session-to"
            type="date"
            value={activeTo}
            min={activeFrom || undefined}
            onChange={(event) => setActiveTo(event.target.value)}
            className="h-8 text-xs"
          />
        </div>
      </div>

      <div className="grid gap-1.5">
        <Label className="text-xs">
          {t("sessionManager.advanced.messageRoles")}
        </Label>
        <MultiSelect
          label={t("sessionManager.advanced.messageRoles")}
          allLabel={t("sessionManager.advanced.allRoles")}
          options={roleOptions}
          selected={roles}
          onChange={setRoles}
        />
      </div>

      <div className="grid gap-1.5">
        <Label className="text-xs">
          {t("sessionManager.advanced.messageKinds")}
        </Label>
        <MultiSelect
          label={t("sessionManager.advanced.messageKinds")}
          allLabel={t("sessionManager.advanced.allKinds")}
          options={messageKindOptions}
          selected={messageKinds}
          onChange={(selected) =>
            setMessageKinds(selected as SessionMessageKind[])
          }
        />
      </div>

      {(validationError || error || progressLabel) && (
        <p
          className={
            validationError || error
              ? "text-xs text-destructive"
              : "text-xs text-muted-foreground"
          }
        >
          {validationError || error || progressLabel}
        </p>
      )}
    </form>
  );
});
