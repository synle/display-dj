import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import VolumeControl from "./VolumeControl";

describe("VolumeControl", () => {
  it("renders a slider with the given volume value", () => {
    render(<VolumeControl value={75} onChange={() => {}} />);
    const slider = screen.getByRole("slider");
    expect(slider).toHaveValue("75");
  });

  it("shows muted icon when volume is 0", () => {
    render(<VolumeControl value={0} onChange={() => {}} />);
    expect(screen.getByText("\uD83D\uDD07")).toBeInTheDocument();
  });

  it("shows speaker icon when volume is above 0", () => {
    render(<VolumeControl value={50} onChange={() => {}} />);
    expect(screen.getByText("\uD83D\uDD0A")).toBeInTheDocument();
  });
});
