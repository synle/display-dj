import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import AllMonitorsControl from "./AllMonitorsControl";

describe("AllMonitorsControl", () => {
  it("renders 'All Monitors' label with count", () => {
    render(
      <AllMonitorsControl
        brightness={50}
        onBrightnessChange={() => {}}
        monitorCount={3}
      />
    );
    expect(screen.getByText("All Monitors (3)")).toBeInTheDocument();
  });

  it("renders brightness slider with correct value", () => {
    render(
      <AllMonitorsControl
        brightness={70}
        onBrightnessChange={() => {}}
        monitorCount={2}
      />
    );
    const sliders = screen.getAllByRole("slider");
    expect(sliders).toHaveLength(1);
    expect(sliders[0]).toHaveValue("70");
  });
});
