import {
  Bot,
  Braces,
  Code2,
  Gem,
  Grid2X2,
  MessageSquareText,
  Orbit,
  Pi as PiIcon,
  Sparkles,
  type LucideIcon,
} from "lucide-react";
import { cn } from "@/lib/utils";

const PROVIDER_ICONS: Record<string, LucideIcon> = {
  all: Grid2X2,
  claude: MessageSquareText,
  codex: Code2,
  gemini: Gem,
  grokbuild: Sparkles,
  hermes: Orbit,
  openclaw: Bot,
  opencode: Braces,
  pi: PiIcon,
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
  const Icon = PROVIDER_ICONS[providerId] ?? Code2;

  return (
    <Icon
      aria-hidden="true"
      className={cn("shrink-0 text-muted-foreground", className)}
      size={size}
    />
  );
}
