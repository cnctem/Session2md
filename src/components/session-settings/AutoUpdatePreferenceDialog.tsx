import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { RefreshCw } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  session2mdSettingsApi,
  type Session2mdSettingsSnapshot,
} from "@/lib/api/session2mdSettings";
import { session2mdSettingsKey } from "@/lib/query/session2mdSettings";

interface AutoUpdatePreferenceDialogProps {
  settings: Session2mdSettingsSnapshot | null;
}

export function AutoUpdatePreferenceDialog({
  settings,
}: AutoUpdatePreferenceDialogProps) {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [pendingChoice, setPendingChoice] = useState<boolean | null>(null);

  const saveMutation = useMutation({
    mutationFn: (autoCheckUpdates: boolean) => {
      if (!settings) {
        throw new Error("Session2md settings are not loaded");
      }

      const { resolvedDirectories: _, ...payload } = settings;
      return session2mdSettingsApi.save({
        ...payload,
        autoCheckUpdates,
      });
    },
    onSuccess: (snapshot) => {
      queryClient.setQueryData(session2mdSettingsKey, snapshot);
    },
  });

  const choose = async (autoCheckUpdates: boolean) => {
    setPendingChoice(autoCheckUpdates);
    try {
      await saveMutation.mutateAsync(autoCheckUpdates);
    } catch (error) {
      console.error(
        "[AutoUpdatePreferenceDialog] Failed to save preference",
        error,
      );
      toast.error(t("sessionSettings.autoUpdatePrompt.saveFailed"));
    } finally {
      setPendingChoice(null);
    }
  };

  const isOpen = settings?.autoCheckUpdates === null;

  return (
    <Dialog open={isOpen} onOpenChange={() => undefined}>
      <DialogContent
        className="max-w-md"
        zIndex="top"
        onEscapeKeyDown={(event) => event.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <RefreshCw className="size-5 text-blue-500" />
            {t("sessionSettings.autoUpdatePrompt.title")}
          </DialogTitle>
          <DialogDescription className="whitespace-pre-line leading-relaxed">
            {t("sessionSettings.autoUpdatePrompt.description")}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            disabled={pendingChoice !== null}
            onClick={() => void choose(false)}
          >
            {pendingChoice === false && (
              <RefreshCw className="size-3.5 animate-spin" />
            )}
            {t("sessionSettings.autoUpdatePrompt.no")}
          </Button>
          <Button
            type="button"
            disabled={pendingChoice !== null}
            onClick={() => void choose(true)}
          >
            {pendingChoice === true && (
              <RefreshCw className="size-3.5 animate-spin" />
            )}
            {t("sessionSettings.autoUpdatePrompt.yes")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
