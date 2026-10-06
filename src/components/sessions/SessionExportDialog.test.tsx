import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  SessionExportDialog,
  type SessionExportOptions,
} from "./SessionExportDialog";

const initialOptions: SessionExportOptions = {
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

    const checkboxes = screen.getAllByRole("checkbox");
    fireEvent.click(checkboxes[0]);
    fireEvent.click(
      screen.getByRole("button", {
        name: "sessionManager.exportOptions.confirm",
      }),
    );

    expect(onConfirm).toHaveBeenCalledWith({
      includeThinking: true,
      includeToolInputs: true,
      includeToolOutputs: false,
    });
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

    const initialCheckboxes = screen.getAllByRole("checkbox");
    fireEvent.click(initialCheckboxes[0]);
    expect(initialCheckboxes[0]).toBeChecked();

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

    const reopenedCheckboxes = screen.getAllByRole("checkbox");
    expect(reopenedCheckboxes[0]).not.toBeChecked();
    expect(reopenedCheckboxes[1]).toBeChecked();
    expect(reopenedCheckboxes[2]).not.toBeChecked();
  });
});
