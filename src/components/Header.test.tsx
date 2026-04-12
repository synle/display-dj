import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import Header from "./Header";

describe("Header", () => {
  it("renders the app title", () => {
    render(<Header version="2.0.0" expanded={false} onToggle={() => {}} />);
    expect(screen.getByText("Display DJ")).toBeInTheDocument();
  });

  it("displays the version when provided", () => {
    render(<Header version="2.0.0" expanded={false} onToggle={() => {}} />);
    expect(screen.getByText("v2.0.0")).toBeInTheDocument();
  });

  it("hides version when empty string", () => {
    render(<Header version="" expanded={false} onToggle={() => {}} />);
    expect(screen.queryByText(/^v/)).not.toBeInTheDocument();
  });

  it("calls onToggle when the toggle button is clicked", async () => {
    const onToggle = vi.fn();
    const user = userEvent.setup();
    render(<Header version="2.0.0" expanded={false} onToggle={onToggle} />);

    await user.click(screen.getByRole("button"));
    expect(onToggle).toHaveBeenCalledOnce();
  });

  it("shows correct title when collapsed", () => {
    render(<Header version="2.0.0" expanded={false} onToggle={() => {}} />);
    expect(screen.getByRole("button")).toHaveAttribute(
      "title",
      "Show individual monitors"
    );
  });

  it("shows correct title when expanded", () => {
    render(<Header version="2.0.0" expanded={true} onToggle={() => {}} />);
    expect(screen.getByRole("button")).toHaveAttribute(
      "title",
      "Show all monitors control"
    );
  });

  it("applies expanded class to chevron when expanded", () => {
    const { container } = render(
      <Header version="2.0.0" expanded={true} onToggle={() => {}} />
    );
    const chevron = container.querySelector(".chevron");
    expect(chevron).toHaveClass("expanded");
  });

  it("does not apply expanded class to chevron when collapsed", () => {
    const { container } = render(
      <Header version="2.0.0" expanded={false} onToggle={() => {}} />
    );
    const chevron = container.querySelector(".chevron");
    expect(chevron).not.toHaveClass("expanded");
  });
});
