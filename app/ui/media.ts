import * as Net from "@moq/net";
import * as Publish from "@moq/publish";
import * as Watch from "@moq/watch";
import { Signal } from "@moq/signals";
import { SvelteMap } from "svelte/reactivity";
import type { Participant } from "./types";
import { prepareAudioCodecs } from "./audio-codecs";

export class Publisher {
  readonly video = new Signal<Publish.Video.Source | undefined>(undefined);
  readonly audio = new Signal<Publish.Audio.Source | undefined>(undefined);
  readonly connection = new Signal<Net.Connection.Established | undefined>(
    undefined,
  );
  readonly enabled = new Signal(false);
  readonly capture = new Publish.Video.Capture({ source: this.video });
  readonly broadcast: Publish.Broadcast;
  readonly gallery: Publish.Video.Encoder;
  readonly large: Publish.Video.Encoder;
  readonly microphone: Publish.Audio.Encoder;

  constructor(name: string) {
    this.broadcast = new Publish.Broadcast({
      name: Net.Path.from(name),
      connection: this.connection,
      enabled: this.enabled,
      display: this.capture.out.display,
      latencyMax: 1000,
    });
    const common = { codec: "avc1", keyframeInterval: Net.Time.Milli(1000) };
    this.gallery = new Publish.Video.Encoder("video/gallery", {
      broadcast: this.broadcast,
      capture: this.capture,
      enabled: this.enabled,
      config: {
        ...common,
        maxPixels: 640 * 360,
        frameRate: 15,
        maxBitrate: 450_000,
      },
    });
    this.large = new Publish.Video.Encoder("video/large", {
      broadcast: this.broadcast,
      capture: this.capture,
      enabled: this.enabled,
      config: {
        ...common,
        maxPixels: 1920 * 1080,
        frameRate: 30,
        maxBitrate: 3_000_000,
      },
    });
    this.microphone = new Publish.Audio.Encoder("audio", {
      broadcast: this.broadcast,
      source: this.audio,
      enabled: this.enabled,
      codec: { mime: "opus", bitrate: 32_000 },
      sampleRate: 48_000,
      channelCount: 1,
    });
  }

  close() {
    this.enabled.set(false);
    this.gallery.close();
    this.large.close();
    this.microphone.close();
    this.broadcast.close();
    this.capture.close();
  }
}

export class Receiver {
  readonly available = new Signal(false);
  readonly visible = new Signal(false);
  readonly target = new Signal<Watch.Video.Target>({ name: "video/gallery" });
  readonly canvas = new Signal<HTMLCanvasElement | undefined>(undefined);
  readonly broadcast: Watch.Broadcast;
  readonly videoSource: Watch.Video.Source;
  readonly audioSource: Watch.Audio.Source;
  readonly sync: Watch.Sync;
  readonly video: Watch.Video.Decoder;
  readonly audio: Watch.Audio.Decoder;
  readonly emitter: Watch.Audio.Emitter;
  readonly renderer: Watch.Video.Renderer;

  constructor(connection: Net.Connection.Established, name: string) {
    // The call owns one announcement stream for the whole roster.
    this.broadcast = new Watch.Broadcast({
      connection,
      name: Net.Path.from(name),
      enabled: this.available,
      reload: false,
    });
    this.videoSource = new Watch.Video.Source({
      broadcast: this.broadcast,
      target: this.target,
      supported: Watch.Video.Decoder.supported,
    });
    this.audioSource = new Watch.Audio.Source({
      broadcast: this.broadcast,
      supported: Watch.Audio.Decoder.supported,
    });
    this.sync = new Watch.Sync({
      connection,
      latency: Net.Time.Milli(100),
      video: this.videoSource.out.jitter,
      audio: this.audioSource.out.jitter,
    });
    this.video = new Watch.Video.Decoder(this.videoSource, this.sync, {
      enabled: this.visible,
    });
    this.audio = new Watch.Audio.Decoder(this.audioSource, this.sync, {
      enabled: true,
    });
    this.emitter = new Watch.Audio.Emitter(this.audio, { volume: 1 });
    this.renderer = new Watch.Video.Renderer(this.video, {
      canvas: this.canvas,
      visible: "always",
    });
  }

  close() {
    this.visible.set(false);
    this.renderer.close();
    this.emitter.close();
    this.video.close();
    this.audio.close();
    this.sync.close();
    this.videoSource.close();
    this.audioSource.close();
    this.broadcast.close();
  }
}

export async function connectMedia(url: string) {
  // Track selection probes AudioDecoder immediately; initialize the fallback
  // before constructing either publishers or receivers.
  await prepareAudioCodecs();

  return Net.Connection.connect(new URL(url), {
    websocket: { delay: 0 },
    signal: AbortSignal.timeout(5000),
  });
}

export class CallMedia {
  readonly receivers = new SvelteMap<string, Receiver>();
  readonly publisher: Publisher;
  private announcements: Net.Announce.Consumer;
  private available = new Set<string>();
  private closed = false;

  constructor(
    private connection: Net.Connection.Established,
    broadcast: string,
  ) {
    this.publisher = new Publisher(broadcast);
    this.publisher.connection.set(connection);
    this.publisher.enabled.set(true);
    this.announcements = connection.announced(Net.Path.empty());
    void this.discover();
  }

  private async discover() {
    try {
      for (;;) {
        const event = await this.announcements.next();
        if (!event || this.closed) break;
        if (event.active) this.available.add(event.path);
        else this.available.delete(event.path);
        this.receivers.get(event.path)?.available.set(event.active);
      }
    } catch {
      // The Rust session owns reconnection and supplies a replacement bridge.
    }
  }

  sources(video?: MediaStream, audio?: MediaStream) {
    this.publisher.video.set(
      video?.getVideoTracks()[0] as Publish.Video.Source | undefined,
    );
    const track = audio?.getAudioTracks()[0] as
      Publish.Audio.StreamTrack | undefined;
    // The voice preset enables DTX, which the Opus fallback cannot encode.
    // Default Opus settings work with both native and fallback encoders.
    this.publisher.audio.set(track);
  }

  roster(participants: Participant[], me: string) {
    const wanted = new Set(
      participants
        .filter((person) => person.id !== me)
        .map((person) => person.broadcast),
    );
    for (const [name, receiver] of this.receivers) {
      if (!wanted.has(name)) {
        receiver.close();
        this.receivers.delete(name);
      }
    }
    for (const name of wanted) {
      if (this.receivers.has(name)) continue;
      const receiver = new Receiver(this.connection, name);
      receiver.available.set(this.available.has(name));
      this.receivers.set(name, receiver);
    }
  }

  visible(participants: Participant[], pin: string) {
    const names = new Set(
      participants
        .filter((person) => person.media.camera)
        .map((person) => person.broadcast),
    );
    for (const [name, receiver] of this.receivers) {
      receiver.visible.set(names.has(name));
      receiver.target.set({
        name: name === pin ? "video/large" : "video/gallery",
      });
    }
  }

  diagnostics() {
    return {
      transport: this.connection.transport,
      version: this.connection.version,
      announced: this.available.size,
      publishing: {
        camera: this.publisher.video.peek()?.readyState,
        capturedFrame: this.publisher.capture.out.frame.peek()?.timestamp,
        microphoneContext:
          this.publisher.microphone.out.root.peek()?.context.state,
        microphoneActive: this.publisher.microphone.out.active.peek(),
        microphoneCatalog: this.publisher.microphone.out.catalog.peek(),
        gallery: this.publisher.gallery.out.stats.peek(),
        large: this.publisher.large.out.stats.peek(),
        audio: this.publisher.microphone.out.stats.peek(),
      },
      receiving: [...this.receivers.values()].map((receiver) => ({
        broadcast: receiver.broadcast.in.name.peek(),
        announced: receiver.available.peek(),
        status: receiver.broadcast.out.status.peek(),
        videoTrack: receiver.videoSource.out.track.peek(),
        videoError: receiver.videoSource.out.error.peek(),
        video: receiver.video.out.stats.peek(),
        audioTrack: receiver.audioSource.out.track.peek(),
        audioCatalog: receiver.broadcast.out.catalog.peek()?.audio ?? null,
        audioAvailable: receiver.audioSource.out.available.peek(),
        audioConfig: receiver.audioSource.out.config.peek(),
        audioEnabled: receiver.audio.in.enabled.peek(),
        audio: receiver.audio.out.stats.peek(),
        audioContext: receiver.audio.out.context.peek()?.state,
        audioGraphReady: !!receiver.audio.out.root.peek(),
        audioStalled: receiver.audio.out.stalled.peek(),
        audioTimestamp: receiver.audio.out.timestamp.peek(),
      })),
    };
  }

  close() {
    this.closed = true;
    this.announcements.close();
    this.receivers.forEach((receiver) => receiver.close());
    this.receivers.clear();
    this.publisher.close();
    this.connection.close();
  }
}
