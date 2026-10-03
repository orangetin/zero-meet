<script lang="ts">
  import { MicOff, Pin, PinOff } from "lucide-svelte";
  import { Effect, type Getter } from "@moq/signals";
  import AudioWaveform from "./AudioWaveform.svelte";
  import type { Participant } from "./types";
  import type { Receiver } from "./media";
  let {
    person,
    receiver,
    local,
    localAudio,
    pinned = false,
    featured = false,
    onpin,
  }: {
    person: Participant;
    receiver?: Receiver;
    local?: MediaStream;
    localAudio?: Getter<AudioNode | undefined>;
    pinned?: boolean;
    featured?: boolean;
    onpin: () => void;
  } = $props();
  let canvas = $state<HTMLCanvasElement>();
  let video = $state<HTMLVideoElement>();
  let aspectRatio = $state(16 / 9);
  let speaking = $state(false);

  $effect(() => {
    const current = receiver;
    if (!current) return;

    const signals = new Effect();
    signals.run((effect) => {
      const display = effect.get(current.video.out.display);
      if (display?.width && display.height)
        aspectRatio = display.width / display.height;
    });

    return () => signals.close();
  });

  function resizeVideo() {
    if (video?.videoWidth && video.videoHeight)
      aspectRatio = video.videoWidth / video.videoHeight;
  }

  $effect(() => {
    const current = receiver;
    current?.canvas.set(canvas);
    return () => current?.canvas.set(undefined);
  });
  $effect(() => {
    if (video) {
      video.srcObject = local ?? null;
      if (local) void video.play().catch(() => {});
    }
  });
</script>

<article
  class:featured
  class:speaking={person.media.microphone && speaking}
  style:aspect-ratio={aspectRatio}
  style:--aspect-ratio={aspectRatio}
>
  <div class="avatar" aria-hidden="true">
    {person.name.trim().slice(0, 2).toUpperCase()}
  </div>
  {#if local}
    <video
      bind:this={video}
      onloadedmetadata={resizeVideo}
      onresize={resizeVideo}
      autoplay
      muted
      playsinline
      aria-label="Your camera"
      class="self"
    ></video>
  {:else}
    <canvas
      bind:this={canvas}
      class:visible={person.media.camera}
      aria-label={`${person.name}'s camera`}
    ></canvas>
  {/if}
  <div class="tile-label">
    <div class="participant-name">
      <span>{person.name}</span>
      {#if !person.media.microphone}
        <MicOff size={15} aria-label="Microphone muted" />
      {/if}
    </div>
    {#if person.media.microphone}
      <span class="participant-audio" class:speaking>
        <AudioWaveform
          root={localAudio ?? receiver?.audio.out.root}
          speakingOnly
          onspeakingchange={(value) => (speaking = value)}
        />
      </span>
    {/if}
  </div>
  <button
    class="pin"
    onclick={onpin}
    aria-label={pinned ? `Unpin ${person.name}` : `Pin ${person.name}`}
    aria-pressed={pinned}
    title={pinned ? "Unpin" : "Pin"}
  >
    {#if pinned}
      <PinOff size={17} />
    {:else}
      <Pin size={17} />
    {/if}
  </button>
</article>

<style>
  article {
    --tile-height: var(--row-height, 10000px);
    position: relative;
    width: min(100%, calc(var(--tile-height, 10000px) * var(--aspect-ratio)));
    min-height: 0;
    overflow: hidden;
    border: 2px solid var(--ink);
    border-radius: 12px;
    background: #dddace;
  }
  .avatar {
    height: 100%;
    display: grid;
    place-items: center;
    font-size: clamp(26px, 4vw, 64px);
    font-weight: 700;
    color: #626455;
  }
  article.speaking {
    border-color: #739b40;
    box-shadow: 0 0 0 2px #739b40;
  }
  canvas,
  video {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    background: #232720;
  }
  canvas {
    opacity: 0;
  }
  canvas.visible {
    opacity: 1;
  }
  .self {
    transform: scaleX(-1);
  }
  .tile-label {
    position: absolute;
    bottom: 12px;
    left: 12px;
    max-width: calc(100% - 40px);
    display: grid;
    justify-items: center;
    padding: 6px 10px;
    border: 1.5px solid var(--ink);
    border-radius: 5px;
    background: var(--paper);
    font-size: 13px;
    font-weight: 600;
  }
  .tile-label span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .participant-name {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    max-width: 100%;
  }
  .participant-audio {
    width: 0;
    height: 0;
    overflow: hidden;
  }
  .participant-audio.speaking {
    width: 24px;
    height: 12px;
    margin-top: 3px;
  }
  .pin {
    position: absolute;
    top: 10px;
    right: 10px;
    min-height: 34px;
    padding: 7px;
    box-shadow: 2px 2px 0 var(--ink);
  }
  .featured {
    --tile-height: calc(2 * var(--row-height) + var(--tile-gap));
    grid-column: span 2;
    grid-row: span 2;
  }
</style>
