import { Code2, Grid2X2 } from "lucide-react";
import {
  SESSION_PROVIDER_ICON_ASSETS,
  type SessionProviderIconAsset,
} from "./sessionProviderIcons";
import { cn } from "@/lib/utils";

const getProviderAsset = (
  providerId: string,
): SessionProviderIconAsset | undefined =>
  Object.prototype.hasOwnProperty.call(SESSION_PROVIDER_ICON_ASSETS, providerId)
    ? SESSION_PROVIDER_ICON_ASSETS[
        providerId as keyof typeof SESSION_PROVIDER_ICON_ASSETS
      ]
    : undefined;

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
  const asset = getProviderAsset(providerId);

  if (!asset) {
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

  const sizeValue = typeof size === "number" ? `${size}px` : size;

  if (asset.kind === "image") {
    return (
      <img
        src={asset.url}
        alt={asset.name}
        title={asset.name}
        className={cn(
          "inline-flex shrink-0 items-center justify-center object-contain",
          className,
        )}
        style={{ width: sizeValue, height: sizeValue }}
        loading="lazy"
      />
    );
  }

  return (
    <span
      className={cn(
        "inline-flex shrink-0 items-center justify-center",
        className,
      )}
      title={asset.name}
      style={{
        width: sizeValue,
        height: sizeValue,
        fontSize: sizeValue,
        lineHeight: 1,
      }}
      dangerouslySetInnerHTML={{ __html: asset.content }}
    />
  );
}
