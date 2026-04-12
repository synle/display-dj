import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import MonitorControl from "./MonitorControl";
import { Monitor } from "../types";

const externalMonitor: Monitor = {
  id: "external-1",
  name: "Dell U2723QE",
  brightness: 80,
  contrast: 50,
  supportsBrightness: true,
  supportsContrast: true,
  isBuiltIn: false,
};

const builtInMonitor: Monitor = {
  id: "builtin-0",
  name: "Built-in Display",
  brightness: 60,
  contrast: 50,
  supportsBrightness: true,
  supportsContrast: false,
  isBuiltIn: true,
};

describe("MonitorControl", () => {
  it("renders the monitor name", () => {
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onContrastChange={() => {}}
        onRename={() => {}}
      />
    );
    expect(screen.getByText("Dell U2723QE")).toBeInTheDocument();
  });

  it("renders brightness slider", () => {
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onContrastChange={() => {}}
        onRename={() => {}}
      />
    );
    const sliders = screen.getAllByRole("slider");
    expect(sliders.length).toBeGreaterThanOrEqual(1);
    expect(sliders[0]).toHaveValue("80");
  });

  it("renders contrast slider when supported", () => {
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onContrastChange={() => {}}
        onRename={() => {}}
      />
    );
    const sliders = screen.getAllByRole("slider");
    expect(sliders).toHaveLength(2);
  });

  it("hides contrast slider when not supported", () => {
    render(
      <MonitorControl
        monitor={builtInMonitor}
        onBrightnessChange={() => {}}
        onContrastChange={() => {}}
        onRename={() => {}}
      />
    );
    const sliders = screen.getAllByRole("slider");
    expect(sliders).toHaveLength(1);
  });

  it("enters edit mode on name click", async () => {
    const user = userEvent.setup();
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onContrastChange={() => {}}
        onRename={() => {}}
      />
    );

    await user.click(screen.getByText("Dell U2723QE"));
    const input = screen.getByRole("textbox");
    expect(input).toBeInTheDocument();
    expect(input).toHaveValue("Dell U2723QE");
  });

  it("calls onRename when editing is confirmed with Enter", async () => {
    const onRename = vi.fn();
    const user = userEvent.setup();
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onContrastChange={() => {}}
        onRename={onRename}
      />
    );

    await user.click(screen.getByText("Dell U2723QE"));
    const input = screen.getByRole("textbox");
    await user.clear(input);
    await user.type(input, "My Monitor{Enter}");
    expect(onRename).toHaveBeenCalledWith("My Monitor");
  });

  it("cancels editing on Escape", async () => {
    const onRename = vi.fn();
    const user = userEvent.setup();
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onContrastChange={() => {}}
        onRename={onRename}
      />
    );

    await user.click(screen.getByText("Dell U2723QE"));
    const input = screen.getByRole("textbox");
    await user.clear(input);
    await user.type(input, "New Name{Escape}");
    expect(onRename).not.toHaveBeenCalled();
    expect(screen.getByText("Dell U2723QE")).toBeInTheDocument();
  });

  it("does not call onRename when name is unchanged", async () => {
    const onRename = vi.fn();
    const user = userEvent.setup();
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onContrastChange={() => {}}
        onRename={onRename}
      />
    );

    await user.click(screen.getByText("Dell U2723QE"));
    const input = screen.getByRole("textbox");
    await user.type(input, "{Enter}");
    expect(onRename).not.toHaveBeenCalled();
  });

  it("shows monitor icon for external display", () => {
    render(
      <MonitorControl
        monitor={externalMonitor}
        onBrightnessChange={() => {}}
        onContrastChange={() => {}}
        onRename={() => {}}
      />
    );
    expect(screen.getByText("\uD83D\uDDA5")).toBeInTheDocument();
  });

  it("shows laptop icon for built-in display", () => {
    render(
      <MonitorControl
        monitor={builtInMonitor}
        onBrightnessChange={() => {}}
        onContrastChange={() => {}}
        onRename={() => {}}
      />
    );
    expect(screen.getByText("\uD83D\uDCBB")).toBeInTheDocument();
  });
});
