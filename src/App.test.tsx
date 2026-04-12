import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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
            uid: "builtin-0::Built-in Display",
            name: "Built-in Display",
            originalName: "Built-in Display",
            brightness: 50,
            supportsBrightness: true,
            isBuiltIn: true,
          },
        ]);
      case "get_dark_mode":
        return Promise.resolve(false);
      case "get_volume":
        return Promise.resolve(50);
      case "get_preferences":
        return Promise.resolve({
          showIndividualDisplays: false,
          minBrightness: 10,
          keyBindings: [],
          profiles: [
            {
              name: "Presentation",
              command: [
                "command/changeBrightness/100",
                "command/changeDarkMode/light",
                "command/changeVolume/50",
              ],
            },
            {
              name: "Focus",
              command: [
                "command/changeBrightness/80",
                "command/changeDarkMode/dark",
                "command/changeVolume/30",
              ],
            },
            {
              name: "Daylight",
              command: [
                "command/changeBrightness/100",
                "command/changeDarkMode/light",
                "command/changeVolume/100",
              ],
            },
          ],
          nightModeSchedule: {
            enabled: false,
            nightStart: "21:00",
            nightBrightness: 20,
            dayStart: "07:00",
            dayBrightness: 100,
          },
          debugLogging: false,
          launchAtLogin: false,
          monitorConfigs: [],
        });
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

  it("renders profile buttons", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText("Presentation")).toBeInTheDocument();
      expect(screen.getByText("Focus")).toBeInTheDocument();
      expect(screen.getByText("Daylight")).toBeInTheDocument();
    });
  });

  it("shows all-monitors view by default (collapsed)", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText("All Monitors (1)")).toBeInTheDocument();
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

  it("renders collapsed and expanded views without JS errors", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    const user = userEvent.setup();

    render(<App />);

    // Wait for initial data to load (collapsed view)
    await waitFor(() => {
      expect(screen.getByText("All Monitors (1)")).toBeInTheDocument();
    });

    // Expand to show individual monitors
    await user.click(screen.getByTitle("Show individual monitors"));
    await waitFor(() => {
      expect(screen.getByText("Built-in Display")).toBeInTheDocument();
    });

    // Collapse back
    await user.click(screen.getByTitle("Show all monitors control"));
    await waitFor(() => {
      expect(screen.getByText("All Monitors (1)")).toBeInTheDocument();
    });

    // No console.error calls should have occurred
    expect(errorSpy).not.toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  it("renders with multiple monitors without JS errors", async () => {
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    mockInvoke.mockImplementation((cmd: string) => {
      switch (cmd) {
        case "get_monitors":
          return Promise.resolve([
            {
              id: "builtin-0",
              uid: "builtin-0::Built-in Display",
              name: "Built-in Display",
              originalName: "Built-in Display",
              brightness: 100,
              supportsBrightness: true,
              isBuiltIn: true,
            },
            {
              id: "1",
              uid: "1::Dell U2723QE",
              name: "Dell U2723QE",
              originalName: "Dell U2723QE",
              brightness: 80,
              supportsBrightness: true,
              isBuiltIn: false,
            },
            {
              id: "2",
              uid: "2::LG 27UK850",
              name: "",
              originalName: "LG 27UK850",
              brightness: 60,
              supportsBrightness: true,
              isBuiltIn: false,
            },
          ]);
        case "get_dark_mode":
          return Promise.resolve(true);
        case "get_volume":
          return Promise.resolve(75);
        case "get_app_version":
          return Promise.resolve("2.0.0");
        default:
          return Promise.resolve(undefined);
      }
    });

    const user = userEvent.setup();
    render(<App />);

    // Collapsed view with 3 monitors
    await waitFor(() => {
      expect(screen.getByText("All Monitors (3)")).toBeInTheDocument();
    });

    // Expand to individual monitors
    await user.click(screen.getByTitle("Show individual monitors"));
    await waitFor(() => {
      expect(screen.getByText("Built-in Display")).toBeInTheDocument();
      expect(screen.getByText("Dell U2723QE")).toBeInTheDocument();
      // Monitor with empty name should fall back to originalName
      expect(screen.getByText("LG 27UK850")).toBeInTheDocument();
    });

    expect(errorSpy).not.toHaveBeenCalled();
    errorSpy.mockRestore();
  });
});
