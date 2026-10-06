import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export interface SessionExportOptions {
  includeThinking: boolean;
  includeToolInputs: boolean;
  includeToolOutputs: boolean;
}

interface SessionExportDialogProps {
  open: boolean;
  options: SessionExportOptions;
  onOpenChange: (open: boolean) => void;
  onConfirm: (options: SessionExportOptions) => void;
}

export function SessionExportDialog({
  open,
  options,
  onOpenChange,
  onConfirm,
}: SessionExportDialogProps) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState(options);

  useEffect(() => {
    if (open) setDraft(options);
  }, [open, options]);

  const optionRows = [
    {
      key: "includeThinking" as const,
      label: t("sessionSettings.exportContent.includeThinking.label"),
      description: t(
        "sessionSettings.exportContent.includeThinking.description",
      ),
    },
    {
      key: "includeToolInputs" as const,
      label: t("sessionSettings.exportContent.includeToolInputs.label"),
      description: t(
        "sessionSettings.exportContent.includeToolInputs.description",
      ),
    },
    {
      key: "includeToolOutputs" as const,
      label: t("sessionSettings.exportContent.includeToolOutputs.label"),
      description: t(
        "sessionSettings.exportContent.includeToolOutputs.description",
      ),
    },
  ];

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg" zIndex="nested">
        <DialogHeader>
          <DialogTitle>{t("sessionManager.exportOptions.title")}</DialogTitle>
          <DialogDescription>
            {t("sessionManager.exportOptions.description")}
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-1 px-6 py-5">
          {optionRows.map((option) => (
            <label
              key={option.key}
              className="flex cursor-pointer items-start gap-3 rounded-lg px-2 py-2.5 transition-colors hover:bg-muted/60"
            >
              <Checkbox
                className="mt-0.5"
                checked={draft[option.key]}
                onCheckedChange={(checked) =>
                  setDraft((current) => ({
                    ...current,
                    [option.key]: checked,
                  }))
                }
              />
              <span className="space-y-1">
                <span className="block text-sm font-medium">
                  {option.label}
                </span>
                <span className="block text-xs text-muted-foreground">
                  {option.description}
                </span>
              </span>
            </label>
          ))}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button onClick={() => onConfirm(draft)}>
            {t("sessionManager.exportOptions.confirm")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
