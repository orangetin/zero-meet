import { mockIPC } from "@tauri-apps/api/mocks";
import type { Channel } from "@tauri-apps/api/core";
import type { MeetingEvent } from "../ui/types";

Object.defineProperty(navigator.mediaDevices, "getUserMedia", {
  value: async () => {
    throw new DOMException("Denied for UI test", "NotAllowedError");
  },
});
mockIPC(
  (command, args) => {
    if (command === "plugin:deep-link|get_current") {
      const invite = new URL(location.href).searchParams.get("invite");
      return invite ? [invite] : null;
    }
    if (command === "create_room") return "zero-meet://test-room";
    if (command === "inspect_invite") {
      if (args?.invite === "bad")
        throw new Error("This invitation is invalid.");
      return args?.invite;
    }
    if (command === "join_room") {
      const events = args?.events as Channel<MeetingEvent>;
      events.onmessage({
        type: "roster",
        roster: Array.from({ length: 32 }, (_, index) => ({
          id: String(index),
          name: index === 0 ? String(args?.name) : `Guest ${index + 1}`,
          broadcast: `guest/${index}`,
          media: { microphone: index % 2 === 0, camera: false },
        })),
      });
      events.onmessage({ type: "reconnecting" });
    }
    return null;
  },
  { shouldMockEvents: true },
);

await import("../ui/main");
