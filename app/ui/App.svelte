<script lang="ts">
  import { onMount } from "svelte";
  import { Channel, invoke } from "@tauri-apps/api/core";
  import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
  import {
    ArrowRight,
    X,
    ChevronLeft,
    ChevronRight,
    Mic,
    MicOff,
    Video,
    VideoOff,
    PhoneOff,
    Settings2,
    Users,
    Plus,
  } from "lucide-svelte";
  import { Capture } from "./capture.svelte";
  import { CallMedia, connectMedia } from "./media";
  import { MediaDiagnostics } from "./media-diagnostics";
  import type { MeetingEvent, Participant } from "./types";
  import Tile from "./Tile.svelte";
  import ParticipantGrid from "./ParticipantGrid.svelte";
  import DeviceSettings from "./DeviceSettings.svelte";
  import InvitationButton from "./InvitationButton.svelte";
  import LeaveMeetingDialog from "./LeaveMeetingDialog.svelte";
  import AudioWaveform from "./AudioWaveform.svelte";
  import "./style.css";

  function preferences() {
    try {
      return JSON.parse(localStorage.getItem("preferences") ?? "{}");
    } catch {
      return {};
    }
  }
  const saved = preferences();
  const capture = new Capture();
  const diagnostics = new MediaDiagnostics();
  let name = $state<string>(saved.name ?? "");
  let server = $state<string>(saved.server ?? "");
  let camera = $state<string>(saved.camera ?? "");
  let microphone = $state<string>(saved.microphone ?? "");
  let screen = $state<"home" | "preview" | "call">("home");
  let phase = $state<"joining" | "connected" | "reconnecting" | "failed">(
    "joining",
  );
  let invite = $state("");
  let pasted = $state("");
  let pendingInvite = $state("");
  let error = $state("");
  let busy = $state(false);
  let settings = $state(false);
  let confirmingLeave = $state(false);
  let retryable = $state(false);
  let wantCamera = true;
  let wantMicrophone = false;
  let roster = $state<Participant[]>([]);
  let me = $state("");
  let serverLatency = $state<number | null>(null);
  let page = $state(0);
  let pin = $state("");
  let media = $state.raw<CallMedia>();
  let preview = $state<HTMLVideoElement>();
  let callGeneration = 0;
  let mediaGeneration = 0;

  const pages = $derived(
    Math.max(1, Math.ceil((roster.length - (pin ? 1 : 0)) / (pin ? 8 : 9))),
  );
  const visible = $derived.by(() => {
    const pinned = roster.find((person) => person.broadcast === pin);
    const others = roster.filter((person) => person !== pinned);
    const count = pinned ? 8 : 9;
    return [
      ...(pinned ? [pinned] : []),
      ...others.slice(
        Math.min(page, pages - 1) * count,
        (Math.min(page, pages - 1) + 1) * count,
      ),
    ];
  });
  $effect(() => {
    localStorage.setItem(
      "preferences",
      JSON.stringify({ name, server, camera, microphone }),
    );
  });
  $effect(() => {
    if (preview) {
      preview.srcObject = capture.video ?? null;
      if (capture.video) void preview.play().catch(() => {});
    }
  });
  $effect(() => {
    media?.sources(capture.video, capture.audio);
    if (screen === "call")
      void invoke("set_media", {
        media: { microphone: !!capture.audio, camera: !!capture.video },
      }).catch((cause) => (error = String(cause)));
  });
  $effect(() => {
    media?.roster(roster, me);
  });
  $effect(() => {
    media?.visible(visible, pin);
  });
  $effect(() => {
    if (pin && !roster.some((person) => person.broadcast === pin)) pin = "";
  });

  onMount(() => {
    const stopDiagnostics = diagnostics.start();
    const changed = () => {
      void capture.refreshDevices().catch(() => {});
    };
    navigator.mediaDevices.addEventListener("devicechange", changed);
    const accept = (urls: string[]) => {
      const next = urls.find((url) => url.startsWith("zero-meet://"));
      if (!next) return;
      if (screen === "call") pendingInvite = next;
      else void prepare(next);
    };
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void onOpenUrl(accept).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    void getCurrent().then((urls) => {
      if (!disposed && urls) accept(urls);
    });
    return () => {
      stopDiagnostics();
      disposed = true;
      unlisten?.();
      navigator.mediaDevices.removeEventListener("devicechange", changed);
      media?.close();
      capture.close();
    };
  });

  async function prepare(value: string) {
    busy = true;
    error = "";
    try {
      invite = await invoke<string>("inspect_invite", { invite: value });
      screen = "preview";
      wantCamera = true;
      wantMicrophone = false;
      capture.close();
      void capture.refreshDevices().catch(() => {});
      await capture.camera(true, camera);
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }

  async function create() {
    busy = true;
    error = "";
    try {
      await prepare(await invoke<string>("create_room", { server }));
    } catch (cause) {
      error = String(cause);
    } finally {
      busy = false;
    }
  }

  function clearMedia() {
    ++mediaGeneration;
    serverLatency = null;
    media?.close();
    media = undefined;
  }

  async function join() {
    if (!name.trim()) {
      error = "Add your name before joining.";
      return;
    }
    diagnostics.clear();
    const generation = ++callGeneration;
    screen = "call";
    phase = "joining";
    error = "";
    capture.error = "";
    const events = new Channel<MeetingEvent>();
    events.onmessage = (event) => {
      if (generation !== callGeneration) return;
      if (event.type === "connected") {
        clearMedia();
        const current = mediaGeneration;
        me = event.me;
        roster = event.roster;
        void connectMedia(event.url)
          .then((connection) => {
            if (generation !== callGeneration || current !== mediaGeneration) {
              connection.close();
              return;
            }
            media = new CallMedia(connection, event.broadcast);
            phase = "connected";
          })
          .catch((cause) => {
            if (generation === callGeneration) {
              error = `Could not start media: ${String(cause)}`;
              phase = "failed";
              retryable = true;
              void invoke("leave_room");
              capture.close();
            }
          });
      } else if (event.type === "roster") roster = event.roster;
      else if (event.type === "latency") serverLatency = event.milliseconds;
      else if (event.type === "reconnecting") {
        clearMedia();
        phase = "reconnecting";
      } else {
        clearMedia();
        phase = "failed";
        error = event.message;
        retryable = event.retryable;
        capture.close();
      }
    };
    try {
      await invoke("join_room", {
        invite,
        name: name.trim(),
        media: { microphone: !!capture.audio, camera: !!capture.video },
        events,
      });
    } catch (cause) {
      error = String(cause);
      phase = "failed";
      retryable = true;
      capture.close();
    }
  }

  async function rejoin() {
    phase = "joining";
    await Promise.all([
      capture.camera(wantCamera, camera),
      capture.microphone(wantMicrophone, microphone),
    ]);
    await join();
  }

  async function leave() {
    confirmingLeave = false;
    ++callGeneration;
    clearMedia();
    capture.close();
    roster = [];
    pin = "";
    page = 0;
    settings = false;
    error = "";
    screen = "home";
    await invoke("leave_room");
  }

  function toggleCamera() {
    wantCamera = !capture.video;
    void capture.camera(wantCamera, camera);
  }
  function toggleMicrophone() {
    wantMicrophone = !capture.audio;
    void capture.microphone(wantMicrophone, microphone);
  }

  async function copyDiagnostics() {
    const report = {
      diagnosticsVersion: 3,
      userAgent: navigator.userAgent,
      phase,
      serverLatency,
      participants: roster.map((person) => ({
        broadcast: person.broadcast,
        local: person.id === me,
        microphone: person.media.microphone,
      })),
      microphone: capture.audio?.getAudioTracks().map((track) => ({
        enabled: track.enabled,
        muted: track.muted,
        readyState: track.readyState,
        sampleRate: track.getSettings().sampleRate,
        channelCount: track.getSettings().channelCount,
      })),
      codecs: {
        videoEncoder: typeof VideoEncoder,
        videoDecoder: typeof VideoDecoder,
        audioEncoder: typeof AudioEncoder,
        audioDecoder: typeof AudioDecoder,
      },
      media: media?.diagnostics(),
      mediaEvents: diagnostics.snapshot(),
    };
    console.info("zero-meet diagnostics", report);
    try {
      await navigator.clipboard.writeText(JSON.stringify(report, null, 2));
    } catch {
      error =
        "Diagnostics are in the Web Inspector console; clipboard was unavailable.";
    }
  }
</script>

<div class="shell" class:in-call={screen === "call"}>
  <header>
    <div class="brand">
      <span class="brand-mark"><Video size={24} strokeWidth={2.5} /></span>
      zero-meet
      <span class="beta">BETA</span>
    </div>

    {#if screen === "preview"}
      <button
        class="icon-button"
        aria-label="Close preview"
        title="Close preview"
        onclick={() => void leave()}
      >
        <X size={21} />
      </button>
    {:else if screen === "call"}
      <span class="room-count">
        <span class:live={phase === "connected"} class="status-dot"></span>
        {phase === "connected"
          ? "Connected"
          : phase === "failed"
            ? "Disconnected"
            : "Connecting…"}
        <span class="divider">/</span>
        <Users size={16} />
        <span aria-label={`${roster.length} participants`}>
          {roster.length}
        </span>
        {#if phase === "connected" && serverLatency !== null}
          <span
            class="server-latency"
            title="Round-trip time to the meeting server"
            aria-label={`Server latency ${serverLatency} milliseconds`}
          >
            {serverLatency} ms
          </span>
        {/if}
      </span>
    {/if}
  </header>

  {#if error || capture.error}
    <div class="notice error" role="alert">
      {error || capture.error}
    </div>
  {/if}
  {#if pendingInvite}
    <div class="notice">
      You opened another invitation. <button
        onclick={async () => {
          const next = pendingInvite;
          pendingInvite = "";
          await leave();
          await prepare(next);
        }}
      >
        Leave and open it
      </button>
      <button onclick={() => (pendingInvite = "")}>Dismiss</button>
    </div>
  {/if}

  {#if screen === "home"}
    <main class="home">
      <section class="welcome">
        <h1>
          A room for<br />
          good company<span class="accent-dot">.</span>
        </h1>

        <div class="people-art" aria-hidden="true">
          <span>Hi!</span>
          <span>Hey.</span>
          <span>Hello ☀</span>
        </div>
      </section>

      <section class="entry-card">
        <h2>Start or join a call</h2>
        <form
          onsubmit={(event) => {
            event.preventDefault();
            void create();
          }}
        >
          <label for="server">Your meeting server</label>
          <input
            id="server"
            bind:value={server}
            placeholder="Paste a server address"
            required
            autocomplete="off"
            spellcheck="false"
          />
          <button class="primary full" disabled={busy || !server.trim()}>
            <Plus size={19} />
            {busy ? "Opening room…" : "Create a room"}
          </button>
        </form>
        <div class="or"><span>or</span></div>
        <form
          onsubmit={(event) => {
            event.preventDefault();
            void prepare(pasted);
          }}
        >
          <label for="invitation">Invitation link</label>
          <div class="input-action">
            <input
              id="invitation"
              bind:value={pasted}
              placeholder="zero-meet://…"
              required
              maxlength="4096"
              autocomplete="off"
              spellcheck="false"
            />
            <button
              disabled={busy || !pasted.trim()}
              aria-label="Open invitation"
            >
              <ArrowRight size={22} />
            </button>
          </div>
        </form>
      </section>
    </main>
  {:else if screen === "preview"}
    <main class="prejoin">
      <section class="preview-panel">
        <div class="preview-image">
          <div class="preview-placeholder">
            {name.trim().slice(0, 2).toUpperCase() || "YOU"}
          </div>
          <video
            bind:this={preview}
            autoplay
            muted
            playsinline
            class:shown={!!capture.video}
            aria-label="Your private camera preview"
          ></video>
        </div>
        <div class="preview-controls">{@render controls()}</div>
      </section>
      <section class="ready">
        <h1>Ready when you are.</h1>
        <form
          onsubmit={(event) => {
            event.preventDefault();
            void join();
          }}
        >
          <label for="name">Your name</label>
          <input
            id="name"
            bind:value={name}
            maxlength="64"
            placeholder="Your name"
            autocomplete="given-name"
            required
          />
          <button class="primary full" disabled={busy || !name.trim()}>
            Join room <ArrowRight size={20} />
          </button>
        </form>
      </section>
    </main>
  {:else}
    <main class="call">
      {#if phase !== "connected"}
        <div class="connection-panel" role="status">
          <h2>
            {phase === "joining"
              ? "Pulling up a chair…"
              : phase === "reconnecting"
                ? "Finding our way back…"
                : "You're disconnected."}
          </h2>
          <p>
            {phase === "reconnecting"
              ? "We'll try again for up to 30 seconds. Your choices are saved."
              : phase === "joining"
                ? "Connecting to your meeting server."
                : "You can leave or try joining again."}
          </p>
          {#if phase === "failed" && retryable}
            <button class="primary" onclick={() => void rejoin()}>
              Rejoin room <ArrowRight size={18} />
            </button>
          {/if}
        </div>
      {/if}
      <ParticipantGrid
        count={visible.length}
        featured={!!pin && visible.length > 2}
      >
        {#each visible as person (person.broadcast)}
          <Tile
            {person}
            receiver={media?.receivers.get(person.broadcast)}
            local={person.id === me ? capture.video : undefined}
            localAudio={person.id === me
              ? media?.publisher.microphone.out.root
              : undefined}
            pinned={pin === person.broadcast}
            featured={pin === person.broadcast && visible.length > 2}
            onpin={() => {
              pin = pin === person.broadcast ? "" : person.broadcast;
              page = 0;
            }}
          />
        {/each}
      </ParticipantGrid>
      {#if roster.length === 1 && phase === "connected"}
        <div class="waiting">
          <h2>A little quiet in here.</h2>
          <p>Send your invitation and let the conversation begin.</p>
        </div>
      {/if}
      {#if pages > 1}
        <nav class="pagination" aria-label="Participant pages">
          <button
            aria-label="Previous participants"
            disabled={page === 0}
            onclick={() => (page = Math.max(0, page - 1))}
          >
            <ChevronLeft size={18} />
          </button>
          <span>
            {Math.min(page, pages - 1) + 1} / {pages}
            <small>Everyone stays audible</small>
          </span>
          <button
            aria-label="Next participants"
            disabled={page >= pages - 1}
            onclick={() => page++}
          >
            <ChevronRight size={18} />
          </button>
        </nav>
      {/if}
    </main>
    <footer class="call-controls">
      <InvitationButton invitation={invite} />
      <div class="main-controls">
        {@render controls()}
      </div>
      <button
        class="leave icon-button"
        aria-label="Leave"
        title="Leave call"
        onclick={() => (confirmingLeave = true)}
      >
        <PhoneOff size={21} />
      </button>
    </footer>
  {/if}

  {#if settings && screen !== "home"}
    <DeviceSettings
      {capture}
      bind:camera
      bind:microphone
      inCall={screen === "call"}
      onclose={() => (settings = false)}
      oncopydiagnostics={copyDiagnostics}
    />
  {/if}

  {#if confirmingLeave && screen === "call"}
    <LeaveMeetingDialog
      oncancel={() => (confirmingLeave = false)}
      onconfirm={() => void leave()}
    />
  {/if}
</div>

{#snippet controls()}
  <button
    class="icon-button media-control"
    class:has-indicator={!!capture.audio}
    class:off={!capture.audio}
    disabled={capture.microphoneBusy ||
      (phase === "failed" && screen === "call")}
    aria-pressed={!!capture.audio}
    onclick={toggleMicrophone}
    aria-label={capture.audio ? "Mute microphone" : "Unmute microphone"}
    title={capture.audio ? "Mute microphone" : "Unmute microphone"}
  >
    {#if capture.audio}
      <Mic size={21} />
    {:else}
      <MicOff size={21} />
    {/if}

    {#if capture.audio}
      <span class="microphone-level">
        <AudioWaveform
          stream={capture.audio}
          microphone={media?.publisher.microphone}
          enabled={true}
          hasListeners={roster.length > 1}
        />
      </span>
    {/if}
  </button>

  <button
    class="icon-button media-control"
    class:has-indicator={!!capture.video}
    class:off={!capture.video}
    disabled={capture.cameraBusy || (phase === "failed" && screen === "call")}
    aria-pressed={!!capture.video}
    onclick={toggleCamera}
    aria-label={capture.video ? "Turn camera off" : "Turn camera on"}
    title={capture.video ? "Turn camera off" : "Turn camera on"}
  >
    {#if capture.video}
      <Video size={21} />
      <span class="camera-indicator" aria-hidden="true"></span>
    {:else}
      <VideoOff size={21} />
    {/if}
  </button>

  <button
    class="icon-button"
    title="Settings"
    aria-label="Device settings"
    aria-expanded={settings}
    onclick={() => (settings = !settings)}
  >
    <Settings2 size={20} />
  </button>
{/snippet}
