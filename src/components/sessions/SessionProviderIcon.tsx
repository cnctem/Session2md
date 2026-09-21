import { Code2, Grid2X2 } from "lucide-react";
import { ProviderIcon } from "@/components/ProviderIcon";
import { cn } from "@/lib/utils";

// 与主界面 AppSwitcher / APP_ICON_MAP 相同的品牌图标方案，
// 图标名对应 src/icons/extracted/index.ts 中的注册键。
const PROVIDER_BRAND_ICONS: Record<string, { icon: string; name: string }> = {
  claude: { icon: "claude", name: "Claude Code" },
  codex: { icon: "openai", name: "Codex" },
  gemini: { icon: "gemini", name: "Gemini CLI" },
  grokbuild: { icon: "grok", name: "Grok Build" },
  hermes: { icon: "hermes", name: "Hermes" },
  openclaw: { icon: "openclaw", name: "OpenClaw" },
  opencode: { icon: "opencode", name: "OpenCode" },
  pi: { icon: "pi", name: "Pi" },
  cursor: { icon: "cursor", name: "Cursor" },
  antigravity: { icon: "antigravity", name: "Antigravity" },
  reasonix: { icon: "reasonix", name: "Reasonix" },
  mimocode: { icon: "mimocode", name: "MiMo Code" },
  zcode: { icon: "zcode", name: "ZCode" },
  kimi: { icon: "kimi", name: "Kimi" },
  kilocode: { icon: "kilocode", name: "Kilo Code" },
  qoder: { icon: "qoder", name: "Qoder CLI" },
  workbuddy: { icon: "workbuddy", name: "WorkBuddy" },
  qwen: { icon: "qwen", name: "Qwen Work" },
  continue: { icon: "continue", name: "Continue" },
  cline: { icon: "cline", name: "Cline" },
  goose: { icon: "goose", name: "Goose" },
  zed: { icon: "zed", name: "Zed" },
  crush: { icon: "crush", name: "Crush" },
  teleagent: { icon: "teleagent", name: "TeleAgent" },
  dsh: { icon: "deepseek", name: "DeepSeek" },
};

interface SessionProviderIconProps {
  providerId: string;
  size?: number;
  className?: string;
}

export function SessionProviderIcon({
  providerId,
  size = 16,
  className,
}: SessionProviderIconProps) {
  const brand = PROVIDER_BRAND_ICONS[providerId];

  if (!brand) {
    // “全部”与未知提供方沿用中性线性图标
    const Icon = providerId === "all" ? Grid2X2 : Code2;
    return (
      <Icon
        aria-hidden="true"
        className={cn("shrink-0 text-muted-foreground", className)}
        size={size}
      />
    );
  }

  return (
    <ProviderIcon
      icon={brand.icon}
      name={brand.name}
      size={size}
      showFallback={false}
      className={className}
    />
  );
}
