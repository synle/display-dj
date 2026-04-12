import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import App from "./App";

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockInvoke.mockReset();
  mockInvoke.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_monitors":
        return Promise.resolve([
          {
            id: "builtin-0",
            name: "Built-in Display",
            brightness: 50,
            contrast: 50,
            supportsBrightness: true,
            supportsContrast: false,
            isBuiltIn: true,
          },
        ]);
      case "get_dark_mode":
        return Promise.resolve(false);
      case "get_volume":
        return Promise.resolve(50);
      case "get_app_version":
        return Promise.resolve("2.0.0");
      default:
        return Promise.resolve(undefined);
    }
  });
});

describe("App smoke test", () => {
  it("renders without crashing", () => {
    const { container } = render(<App />);
    expect(container.querySelector(".app")).toBeInTheDocument();
  });

  it("renders the header with title", () => {
    render(<App />);
    expect(screen.getByText("Display DJ")).toBeInTheDocument();
  });

  it("fetches initial data on mount", async () => {
    render(<App />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_monitors");
      expect(mockInvoke).toHaveBeenCalledWith("get_dark_mode");
      expect(mockInvoke).toHaveBeenCalledWith("get_volume");
      expect(mockInvoke).toHaveBeenCalledWith("get_app_version");
    });
  });

  it("displays version from backend", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText("v2.0.0")).toBeInTheDocument();
    });
  });

  it("renders volume control", () => {
    render(<App />);
    const sliders = screen.getAllByRole("slider");
    expect(sliders.length).toBeGreaterThanOrEqual(1);
  });

  it("renders dark mode toggle", () => {
    render(<App />);
    expect(screen.getByText("DARK", { exact: false })).toBeInTheDocument();
    expect(screen.getByText("LIGHT", { exact: false })).toBeInTheDocument();
  });

  it("shows all-monitors view by default (collapsed)", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText("All Monitors")).toBeInTheDocument();
    });
  });

  it("handles backend errors gracefully without crashing", async () => {
    mockInvoke.mockRejectedValue(new Error("backend unavailable"));
    const { container } = render(<App />);
    // App should still render even if all backend calls fail
    await waitFor(() => {
      expect(container.querySelector(".app")).toBeInTheDocument();
    });
  });
});
