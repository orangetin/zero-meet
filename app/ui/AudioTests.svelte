<script lang="ts">
  import { onDestroy } from "svelte";
  import { Mic, Square, Volume2 } from "lucide-svelte";

  let {
    deviceId,
    microphone,
  }: {
    deviceId: string;
    microphone?: MediaStream;
  } = $props();

  let phase = $state<"idle" | "opening" | "recording" | "playing" | "speaker">(
    "idle",
  );
  let error = $state("");
  let generation = 0;
  let context: AudioContext | undefined;
  let stream: MediaStream | undefined;
  let recorder: MediaRecorder | undefined;
  let recordingTimer: ReturnType<typeof setTimeout>;

  const testingMicrophone = $derived(phase !== "idle" && phase !== "speaker");

  async function testMicrophone() {
    if (testingMicrophone) {
      stopTest();
      return;
    }

    error = "";
    phase = "opening";
    const current = ++generation;

    try {
      // Resume during the click so later playback is allowed by the webview.
      const audio = new AudioContext();
      context = audio;
      await audio.resume();
      if (current !== generation) return;

      const input = await openMicrophone();
      if (current !== generation) {
        input.getTracks().forEach((track) => track.stop());
        return;
      }

      stream = input;
      recordMicrophone(input, audio, current);
    } catch {
      failTest(
        current,
        "Could not record your microphone. Check microphone access and the selected device.",
      );
    }
  }

  async function openMicrophone(): Promise<MediaStream> {
    const track = microphone
      ?.getAudioTracks()
      .find((track) => track.readyState === "live");

    // Own a clone so ending a test never stops the call's microphone.
    if (track) return new MediaStream([track.clone()]);

    return navigator.mediaDevices.getUserMedia({
      audio: {
        deviceId: deviceId ? { exact: deviceId } : undefined,
        echoCancellation: true,
        noiseSuppression: true,
      },
    });
  }

  function recordMicrophone(
    input: MediaStream,
    audio: AudioContext,
    current: number,
  ) {
    const recording = new MediaRecorder(input);
    const chunks: Blob[] = [];
    recorder = recording;

    recording.ondataavailable = (event) => {
      if (event.data.size) chunks.push(event.data);
    };

    recording.onerror = () => {
      failTest(current, "Microphone recording stopped. Try testing again.");
    };

    recording.onstop = () => {
      input.getTracks().forEach((track) => track.stop());
      void playRecording(
        new Blob(chunks, { type: recording.mimeType }),
        audio,
        current,
      );
    };

    recording.start();
    phase = "recording";
    recordingTimer = setTimeout(() => {
      if (recording.state === "recording") recording.stop();
    }, 3000);
  }

  async function playRecording(
    recording: Blob,
    audio: AudioContext,
    current: number,
  ) {
    try {
      const bytes = await recording.arrayBuffer();
      if (current !== generation) return;

      const buffer = await audio.decodeAudioData(bytes);
      if (current !== generation) return;

      const playback = new AudioBufferSourceNode(audio, { buffer });
      playback.connect(audio.destination);
      playback.onended = () => {
        if (current === generation) stopTest();
      };

      phase = "playing";
      playback.start();
    } catch {
      failTest(
        current,
        "Could not play your recording. Check your system audio output.",
      );
    }
  }

  async function testSpeakers() {
    error = "";
    phase = "speaker";
    const current = ++generation;

    try {
      const audio = new AudioContext();
      context = audio;
      await audio.resume();
      if (current !== generation) return;

      const tone = new OscillatorNode(audio, { frequency: 440 });
      const gain = new GainNode(audio, { gain: 0 });
      tone.connect(gain).connect(audio.destination);

      const now = audio.currentTime;
      gain.gain.linearRampToValueAtTime(0.08, now + 0.03);
      gain.gain.linearRampToValueAtTime(0, now + 0.6);
      tone.onended = () => {
        if (current === generation) stopTest();
      };

      tone.start(now);
      tone.stop(now + 0.65);
    } catch {
      failTest(
        current,
        "Could not play the test tone. Check your system audio output.",
      );
    }
  }

  function failTest(current: number, message: string) {
    if (current !== generation) return;

    stopTest();
    error = message;
  }

  function stopTest() {
    generation++;
    clearTimeout(recordingTimer);

    if (recorder) {
      recorder.ondataavailable = null;
      recorder.onstop = null;
      recorder.onerror = null;
      if (recorder.state !== "inactive") recorder.stop();
      recorder = undefined;
    }

    stream?.getTracks().forEach((track) => track.stop());
    stream = undefined;

    // Closing an already interrupted context is harmless during teardown.
    void context?.close().catch(() => {});
    context = undefined;
    phase = "idle";
  }

  onDestroy(stopTest);
</script>

<div class="audio-tests">
  <button disabled={phase === "speaker"} onclick={testMicrophone}>
    {#if testingMicrophone}
      <Square size={16} />
      Stop test
    {:else}
      <Mic size={16} />
      Test microphone
    {/if}
  </button>

  <button disabled={phase !== "idle"} onclick={testSpeakers}>
    <Volume2 size={16} />
    {phase === "speaker" ? "Playing tone…" : "Test speakers"}
  </button>
</div>

{#if testingMicrophone}
  <p class="field-help" role="status">
    {#if phase === "opening"}
      Opening microphone…
    {:else if phase === "recording"}
      Speak for 3 seconds…
    {:else if phase === "playing"}
      Playing back…
    {/if}
  </p>
{/if}

{#if error}
  <p class="field-help" role="alert">{error}</p>
{/if}

<style>
  .audio-tests {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }

  button {
    padding: 8px 10px;
    font-size: 13px;
  }
</style>
