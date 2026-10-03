type Level = "debug" | "warn" | "error";

type Entry = {
  at: string;
  level: Level;
  message: string;
};

// MoQ reports subscription and codec failures through the console. Keep a small,
// local history so Copy call diagnostics includes the cause, not only counters.
export class MediaDiagnostics {
  private entries: Entry[] = [];

  start(): () => void {
    const restore: (() => void)[] = [];

    for (const level of ["debug", "warn", "error"] as const) {
      const original = console[level];
      const capture = (...values: unknown[]) => {
        original.apply(console, values);
        this.record(level, values.map(describe).join(" "));
      };

      console[level] = capture;
      restore.push(() => {
        if (console[level] === capture) console[level] = original;
      });
    }

    const onError = (event: ErrorEvent) => {
      this.record("error", describe(event.error ?? event.message));
    };
    const onRejection = (event: PromiseRejectionEvent) => {
      this.record("error", describe(event.reason));
    };

    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);

    return () => {
      restore.forEach((stop) => stop());
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
    };
  }

  clear() {
    this.entries = [];
  }

  snapshot(): Entry[] {
    return this.entries.slice();
  }

  private record(level: Level, message: string) {
    if (
      level === "debug" &&
      !/^(subscribe |publish |encoding audio|received catalog)/.test(message)
    )
      return;

    this.entries.push({
      at: new Date().toISOString(),
      level,
      message: message.slice(0, 2000),
    });

    if (this.entries.length > 60) this.entries.shift();
  }
}

function describe(value: unknown): string {
  if (value instanceof Error || value instanceof DOMException) {
    return `${value.name}: ${value.message}`;
  }

  if (typeof value === "string") return value;

  try {
    return JSON.stringify(value) ?? String(value);
  } catch {
    return String(value);
  }
}
