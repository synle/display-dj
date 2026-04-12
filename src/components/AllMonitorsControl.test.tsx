import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import AllMonitorsControl from "./AllMonitorsControl";

describe("AllMonitorsControl", () => {
  it("renders 'All Monitors' label", () => {
    render(
      <AllMonitorsControl
        brightness={50}
        onBrightnessChange={() => {}}
      />
    );
    expect(screen.getByText("All Monitors")).toBeInTheDocument();
  });

  it("renders brightness slider with correct value", () => {
    render(
      <AllMonitorsControl
        brightness={70}
        onBrightnessChange={() => {}}
      />
    );
    const sliders = screen.getAllByRole("slider");
    expect(sliders).toHaveLength(1);
    expect(sliders[0]).toHaveValue("70");
  });
});
