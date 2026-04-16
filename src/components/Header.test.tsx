import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import Header from "./Header";

const defaultProps = {
  version: "2.0.0",
  expanded: false,
  onToggle: () => {},
  onSettingsToggle: () => {},
  settingsOpen: false,
};

describe("Header", () => {
  it("renders the app title", () => {
    render(<Header {...defaultProps} />);
    expect(screen.getByText("Display DJ")).toBeInTheDocument();
  });

  it("displays the version when provided", () => {
    render(<Header {...defaultProps} />);
    expect(screen.getByText("v2.0.0")).toBeInTheDocument();
  });

  it("hides version when empty string", () => {
    render(<Header {...defaultProps} version="" />);
    expect(screen.queryByText(/^v/)).not.toBeInTheDocument();
  });

  it("calls onToggle when the toggle button is clicked", async () => {
    const onToggle = vi.fn();
    const user = userEvent.setup();
    render(<Header {...defaultProps} onToggle={onToggle} />);

    await user.click(screen.getByTitle("Show individual monitors"));
    expect(onToggle).toHaveBeenCalledOnce();
  });

  it("shows correct title when collapsed", () => {
    render(<Header {...defaultProps} expanded={false} />);
    expect(screen.getByTitle("Show individual monitors")).toBeInTheDocument();
  });

  it("shows correct title when expanded", () => {
    render(<Header {...defaultProps} expanded={true} />);
    expect(screen.getByTitle("Show all monitors control")).toBeInTheDocument();
  });

  it("applies expanded class to chevron when expanded", () => {
    const { container } = render(
      <Header {...defaultProps} expanded={true} />
    );
    const chevron = container.querySelector(".chevron");
    expect(chevron).toHaveClass("expanded");
  });

  it("does not apply expanded class to chevron when collapsed", () => {
    const { container } = render(
      <Header {...defaultProps} expanded={false} />
    );
    const chevron = container.querySelector(".chevron");
    expect(chevron).not.toHaveClass("expanded");
  });

  it("hides the expand/collapse button when settings is open", () => {
    render(<Header {...defaultProps} settingsOpen={true} />);
    expect(
      screen.queryByTitle("Show individual monitors"),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTitle("Show all monitors control"),
    ).not.toBeInTheDocument();
  });

  it("shows the expand/collapse button when settings is closed", () => {
    render(<Header {...defaultProps} settingsOpen={false} />);
    expect(
      screen.getByTitle("Show individual monitors"),
    ).toBeInTheDocument();
  });
});
