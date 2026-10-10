import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  SessionExportDialog,
  type SessionExportOptions,
} from "./SessionExportDialog";

const initialOptions: SessionExportOptions = {
  includeModel: false,
  includeSystem: false,
  includeThinking: false,
  includeToolInputs: true,
  includeToolOutputs: false,
};

describe("SessionExportDialog", () => {
  it("returns the current draft when confirmed", () => {
    const onConfirm = vi.fn();
    render(
      <SessionExportDialog
        open
        options={initialOptions}
        onOpenChange={vi.fn()}
        onConfirm={onConfirm}
      />,
    );

    fireEvent.click(
      screen.getByRole("checkbox", {
        name: /sessionSettings\.exportContent\.includeSystem\.label/,
      }),
    );
    fireEvent.click(
      screen.getByRole("button", {
        name: "sessionManager.exportOptions.confirm",
      }),
    );

    expect(onConfirm).toHaveBeenCalledWith({
      includeModel: false,
      includeSystem: true,
      includeThinking: false,
      includeToolInputs: true,
      includeToolOutputs: false,
    });
  });

  it("renders export options in the expected order", () => {
    render(
      <SessionExportDialog
        open
        options={initialOptions}
        onOpenChange={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );

    const labels = screen.getAllByRole("checkbox").map((checkbox) => {
      const label = checkbox.closest("label");
      return label?.querySelector(".text-sm.font-medium")?.textContent;
    });

    expect(labels).toEqual([
      "sessionSettings.exportContent.includeThinking.label",
      "sessionSettings.exportContent.includeToolInputs.label",
      "sessionSettings.exportContent.includeToolOutputs.label",
      "sessionSettings.exportContent.includeSystem.label",
      "sessionSettings.exportContent.includeModel.label",
    ]);
  });

  it("cancels without confirming", () => {
    const onConfirm = vi.fn();
    const onOpenChange = vi.fn();
    render(
      <SessionExportDialog
        open
        options={initialOptions}
        onOpenChange={onOpenChange}
        onConfirm={onConfirm}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "common.cancel" }));

    expect(onOpenChange).toHaveBeenCalledWith(false);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it("resets the draft from settings whenever it opens", () => {
    const { rerender } = render(
      <SessionExportDialog
        open
        options={initialOptions}
        onOpenChange={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );

    const modelCheckbox = screen.getByRole("checkbox", {
      name: /sessionSettings\.exportContent\.includeModel\.label/,
    });
    fireEvent.click(modelCheckbox);
    expect(modelCheckbox).toBeChecked();

    rerender(
      <SessionExportDialog
        open={false}
        options={initialOptions}
        onOpenChange={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );
    rerender(
      <SessionExportDialog
        open
        options={initialOptions}
        onOpenChange={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );

    const reopenedModelCheckbox = screen.getByRole("checkbox", {
      name: /sessionSettings\.exportContent\.includeModel\.label/,
    });
    expect(reopenedModelCheckbox).not.toBeChecked();
  });
});
