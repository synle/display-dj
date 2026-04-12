import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import AllMonitorsControl from "./AllMonitorsControl";

describe("AllMonitorsControl", () => {
  it("renders 'All Monitors' label", () => {
    render(
      <AllMonitorsControl
        brightness={50}
        contrast={50}
        showContrast={false}
        onBrightnessChange={() => {}}
        onContrastChange={() => {}}
      />
    );
    expect(screen.getByText("All Monitors")).toBeInTheDocument();
  });

  it("renders brightness slider with correct value", () => {
    render(
      <AllMonitorsControl
        brightness={70}
        contrast={50}
        showContrast={false}
        onBrightnessChange={() => {}}
        onContrastChange={() => {}}
      />
    );
    const sliders = screen.getAllByRole("slider");
    expect(sliders).toHaveLength(1);
    expect(sliders[0]).toHaveValue("70");
  });

  it("shows contrast slider when showContrast is true", () => {
    render(
      <AllMonitorsControl
        brightness={50}
        contrast={60}
        showContrast={true}
        onBrightnessChange={() => {}}
        onContrastChange={() => {}}
      />
    );
    const sliders = screen.getAllByRole("slider");
    expect(sliders).toHaveLength(2);
    expect(sliders[1]).toHaveValue("60");
  });

  it("hides contrast slider when showContrast is false", () => {
    render(
      <AllMonitorsControl
        brightness={50}
        contrast={60}
        showContrast={false}
        onBrightnessChange={() => {}}
        onContrastChange={() => {}}
      />
    );
    const sliders = screen.getAllByRole("slider");
    expect(sliders).toHaveLength(1);
  });
});
