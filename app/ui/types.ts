export type MediaState = { microphone: boolean; camera: boolean };
export type Participant = { id: string; name: string; broadcast: string; media: MediaState };
export type MeetingEvent =
  | { type: "connected"; url: string; me: string; broadcast: string; roster: Participant[] }
  | { type: "roster"; roster: Participant[] }
  | { type: "latency"; milliseconds: number | null }
  | { type: "reconnecting" }
  | { type: "failed"; message: string; retryable: boolean };
