<script lang="ts">
  import { ChevronDown, X } from "lucide-svelte";
  import type { Capture } from "./capture.svelte";
  import AudioTests from "./AudioTests.svelte";

  let {
    capture,
    camera = $bindable(""),
    microphone = $bindable(""),
    inCall,
    onclose,
    oncopydiagnostics,
  }: {
    capture: Capture;
    camera?: string;
    microphone?: string;
    inCall: boolean;
    onclose: () => void;
    oncopydiagnostics: () => Promise<void>;
  } = $props();

  function changeCamera() {
    if (capture.video) void capture.camera(true, camera);
  }

  function changeMicrophone() {
    if (capture.audio) void capture.microphone(true, microphone);
  }
</script>

<aside class="settings-panel" aria-label="Device settings">
  <div class="settings-title">
    <h2>Settings</h2>
    <button
      class="icon-button"
      aria-label="Close settings"
      title="Close settings"
      onclick={onclose}
    >
      <X size={21} />
    </button>
  </div>

  <label for="camera">Camera</label>
  <div class="select-field">
    <select id="camera" bind:value={camera} onchange={changeCamera}>
      <option value="">System default</option>
      {#each capture.devices.filter((device) => device.kind === "videoinput") as device}
        <option value={device.deviceId}>{device.label || "Camera"}</option>
      {/each}
    </select>
    <ChevronDown size={18} />
  </div>

  <label for="microphone">Microphone</label>
  <div class="select-field">
    <select id="microphone" bind:value={microphone} onchange={changeMicrophone}>
      <option value="">System default</option>
      {#each capture.devices.filter((device) => device.kind === "audioinput") as device}
        <option value={device.deviceId}>{device.label || "Microphone"}</option>
      {/each}
    </select>
    <ChevronDown size={18} />
  </div>

  {#key microphone}
    <AudioTests deviceId={microphone} microphone={capture.audio} />
  {/key}

  {#if inCall}
    <details class="developer">
      <summary>Developer</summary>
      <button class="text-button" onclick={oncopydiagnostics}>
        Copy call diagnostics
      </button>
    </details>
  {/if}
</aside>

<style>
  .settings-panel {
    position: fixed;
    z-index: 5;
    right: 30px;
    bottom: 100px;
    width: min(380px, calc(100vw - 60px));
    max-height: calc(100dvh - 130px);
    overflow-y: auto;
    padding: 24px;
    background: #fffef9;
    border: 2px solid var(--ink);
    border-radius: 10px;
    box-shadow: 6px 6px 0 var(--ink);
  }

  .settings-title {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 24px;
  }

  h2 {
    margin: 0;
  }

  .select-field {
    position: relative;
    margin-bottom: 20px;
  }

  select {
    appearance: none;
    -webkit-appearance: none;
    padding-right: 40px;
  }

  .select-field :global(svg) {
    position: absolute;
    right: 14px;
    top: 50%;
    transform: translateY(-50%);
    pointer-events: none;
  }

  .developer {
    margin-top: 20px;
    border-top: 1px solid #d8dacf;
    padding-top: 12px;
    font-size: 13px;
    color: #64675b;
  }

  summary {
    cursor: pointer;
  }

  .developer button {
    min-height: 32px;
    padding: 8px 0 0;
    font-size: 13px;
    font-weight: 400;
  }
</style>
