<script lang="ts">
  import { Effect, type Getter } from "@moq/signals";
  import type { Audio } from "@moq/publish";

  let {
    stream,
    microphone,
    root,
    enabled = true,
    hasListeners = false,
    speakingOnly = false,
    onspeakingchange,
  }: {
    stream?: MediaStream;
    microphone?: Audio.Encoder;
    root?: Getter<AudioNode | undefined>;
    enabled?: boolean;
    hasListeners?: boolean;
    speakingOnly?: boolean;
    onspeakingchange?: (speaking: boolean) => void;
  } = $props();

  let waveform = $state("M14,5 L18,5");
  let volume = $state(0);
  let sending = $state(false);
  let speaking = $state(false);

  const description = $derived.by(() => {
    if (!enabled) return "Microphone muted";

    const input = `Audio level: ${Math.round(volume * 100)}%`;
    if (!microphone) return input;
    if (sending) return `${input}. Sending audio.`;
    if (hasListeners) return `${input}. Audio is not being sent.`;

    return `${input}. Microphone on; waiting for someone to join.`;
  });

  $effect(() => {
    const encoder = microphone;
    if (!enabled) return;

    const signals = new Effect();
    const source = root ?? encoder?.out.root;

    if (source) {
      signals.run((effect) => {
        const node = effect.get(source);
        if (node) observeAudio(effect, node, encoder);
      });
    } else if (stream) {
      const context = new AudioContext();
      const root = context.createMediaStreamSource(stream);

      signals.cleanup(() => {
        void context.close();
      });
      signals.run((effect) => observeAudio(effect, root));

      void context.resume().catch((error) => {
        console.warn("Microphone preview meter could not start", error);
      });
    }

    return () => signals.close();
  });

  function observeAudio(
    effect: Effect,
    root: AudioNode,
    encoder?: Audio.Encoder,
  ) {
    const analyser = new AnalyserNode(root.context, { fftSize: 256 });
    const samples = new Float32Array(analyser.fftSize);
    root.connect(analyser);

    let previousFrames = encoder?.out.stats.peek().frames ?? 0;
    let lastFrameAt = -Infinity;
    let lastSpeechAt = -Infinity;

    const timer = setInterval(() => {
      analyser.getFloatTimeDomainData(samples);
      volume = measureVolume(samples);
      waveform = drawWaveform(samples, volume);

      // Ignore quiet background noise and hold through short gaps between words.
      if (volume > 0.3) lastSpeechAt = performance.now();
      updateSpeaking(performance.now() - lastSpeechAt < 300);

      const frames = encoder?.out.stats.peek().frames ?? 0;
      if (frames > previousFrames) lastFrameAt = performance.now();

      previousFrames = frames;
      sending =
        !!encoder?.out.active.peek() && performance.now() - lastFrameAt < 1000;
    }, 50);

    effect.cleanup(() => {
      clearInterval(timer);
      root.disconnect(analyser);
      analyser.disconnect();

      waveform = "M14,5 L18,5";
      volume = 0;
      sending = false;
      updateSpeaking(false);
    });
  }

  function updateSpeaking(value: boolean) {
    if (speaking === value) return;

    speaking = value;
    onspeakingchange?.(value);
  }

  function measureVolume(samples: Float32Array): number {
    const energy = samples.reduce((sum, sample) => sum + sample * sample, 0);
    const rms = Math.sqrt(energy / samples.length);

    // Map the audible range (-60 to 0 dBFS) onto the available line length.
    const decibels = 20 * Math.log10(Math.max(rms, 0.001));

    return Math.max(0, Math.min(1, (decibels + 60) / 60));
  }

  function drawWaveform(samples: Float32Array, level: number): string {
    const width = 4 + level * 24;
    const start = (32 - width) / 2;
    const points = Array.from({ length: 33 }, (_, index) => {
      const sample = samples[Math.floor((index * (samples.length - 1)) / 32)];
      const amplitude = Math.max(-1, Math.min(1, sample * 4));
      const x = start + (index / 32) * width;
      const y = 5 - amplitude * 3;

      return `${index === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
    });

    return points.join(" ");
  }
</script>

<svg
  class="audio-waveform"
  class:muted={!enabled}
  class:quiet={speakingOnly && !speaking}
  viewBox="0 0 32 10"
  role="img"
  aria-label={description}
>
  <title>{description}</title>
  <path d={waveform} />
</svg>

<style>
  .audio-waveform {
    display: block;
    width: 100%;
    height: 100%;
    flex-shrink: 0;
    color: var(--ink);
  }

  .muted {
    opacity: 0.35;
  }

  .quiet {
    opacity: 0;
  }

  path {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.8;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
</style>
