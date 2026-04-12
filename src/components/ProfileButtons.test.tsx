import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, it, expect, vi } from "vitest";
import ProfileButtons from "./ProfileButtons";
import { Profile } from "../types";

const mockProfiles: Profile[] = [
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
];

const extraProfile: Profile = {
  name: "Mute",
  command: "command/changeVolume/0",
};

describe("ProfileButtons", () => {
  it("renders a button for each profile (up to 3)", () => {
    render(<ProfileButtons profiles={mockProfiles} onActivate={() => {}} />);
    expect(screen.getByText("Presentation")).toBeInTheDocument();
    expect(screen.getByText("Focus")).toBeInTheDocument();
    expect(screen.getByText("Daylight")).toBeInTheDocument();
  });

  it("renders nothing when profiles is empty", () => {
    const { container } = render(
      <ProfileButtons profiles={[]} onActivate={() => {}} />
    );
    expect(container.innerHTML).toBe("");
  });

  it("calls onActivate with correct index when clicked", async () => {
    const onActivate = vi.fn();
    const user = userEvent.setup();
    render(
      <ProfileButtons profiles={mockProfiles} onActivate={onActivate} />
    );

    await user.click(screen.getByText("Focus"));
    expect(onActivate).toHaveBeenCalledWith(1);

    await user.click(screen.getByText("Presentation"));
    expect(onActivate).toHaveBeenCalledWith(0);
  });

  it("shows fallback name for unnamed profiles", () => {
    const profiles: Profile[] = [
      { name: "", command: "command/changeVolume/0" },
    ];
    render(<ProfileButtons profiles={profiles} onActivate={() => {}} />);
    expect(screen.getByText("Unnamed Profile #1")).toBeInTheDocument();
  });

  it("shows overflow button when more than 3 profiles", () => {
    render(
      <ProfileButtons
        profiles={[...mockProfiles, extraProfile]}
        onActivate={() => {}}
      />
    );
    expect(screen.getByText("Presentation")).toBeInTheDocument();
    expect(screen.getByText("Focus")).toBeInTheDocument();
    expect(screen.getByText("Daylight")).toBeInTheDocument();
    expect(screen.queryByText("Mute")).not.toBeInTheDocument();
    expect(screen.getByTitle("More profiles")).toBeInTheDocument();
  });

  it("opens overflow menu and activates profile by index", async () => {
    const onActivate = vi.fn();
    const user = userEvent.setup();
    render(
      <ProfileButtons
        profiles={[...mockProfiles, extraProfile]}
        onActivate={onActivate}
      />
    );

    await user.click(screen.getByTitle("More profiles"));
    expect(screen.getByText("Mute")).toBeInTheDocument();

    await user.click(screen.getByText("Mute"));
    expect(onActivate).toHaveBeenCalledWith(3);
    expect(screen.queryByText("Mute")).not.toBeInTheDocument();
  });
});
