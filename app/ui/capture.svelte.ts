export class Capture {
  video = $state.raw<MediaStream>();
  audio = $state.raw<MediaStream>();
  cameraBusy = $state(false);
  microphoneBusy = $state(false);
  error = $state("");
  devices = $state<MediaDeviceInfo[]>([]);
  private cameraGeneration = 0;
  private microphoneGeneration = 0;

  async refreshDevices() {
    this.devices = await navigator.mediaDevices.enumerateDevices();
  }

  async camera(enabled: boolean, device = "") {
    const generation = ++this.cameraGeneration;
    this.video?.getTracks().forEach((track) => track.stop());
    this.video = undefined;
    this.error = "";
    if (!enabled) return;
    this.cameraBusy = true;

    try {
      const stream = await navigator.mediaDevices.getUserMedia({ video: {
        width: { ideal: 1920, max: 1920 }, height: { ideal: 1080, max: 1080 }, frameRate: { ideal: 30, max: 30 },
        ...(device ? { deviceId: { exact: device } } : {}),
      } });
      if (generation !== this.cameraGeneration) { stream.getTracks().forEach((track) => track.stop()); return; }
      this.video = stream;
      stream.getVideoTracks()[0].onended = () => { this.video = undefined; this.error = "Camera disconnected. Choose another camera or continue without video."; };
      await this.refreshDevices();
    } catch (error) {
      if (generation === this.cameraGeneration) this.error = deviceError(error, "camera");
    } finally {
      if (generation === this.cameraGeneration) this.cameraBusy = false;
    }
  }

  async microphone(enabled: boolean, device = "") {
    const generation = ++this.microphoneGeneration;
    this.audio?.getTracks().forEach((track) => track.stop());
    this.audio = undefined;
    this.error = "";
    if (!enabled) return;
    this.microphoneBusy = true;

    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: {
        echoCancellation: true, noiseSuppression: true, channelCount: 1, sampleRate: 48_000,
        ...(device ? { deviceId: { exact: device } } : {}),
      } });
      if (generation !== this.microphoneGeneration) { stream.getTracks().forEach((track) => track.stop()); return; }
      this.audio = stream;
      stream.getAudioTracks()[0].onended = () => { this.audio = undefined; this.error = "Microphone disconnected. Choose another microphone to speak."; };
      await this.refreshDevices();
    } catch (error) {
      if (generation === this.microphoneGeneration) this.error = deviceError(error, "microphone");
    } finally {
      if (generation === this.microphoneGeneration) this.microphoneBusy = false;
    }
  }

  close() {
    ++this.cameraGeneration;
    ++this.microphoneGeneration;
    this.video?.getTracks().forEach((track) => track.stop());
    this.audio?.getTracks().forEach((track) => track.stop());
    this.video = this.audio = undefined;
    this.cameraBusy = this.microphoneBusy = false;
  }
}

function deviceError(error: unknown, device: string) {
  if (error instanceof DOMException && error.name === "NotAllowedError") {
    return `Allow ${device} access in System Settings → Privacy & Security, then try again. You can also join without it.`;
  }
  if (error instanceof DOMException && ["NotFoundError", "OverconstrainedError"].includes(error.name)) {
    return `That ${device} is unavailable. Choose System default or connect another device.`;
  }
  return `Could not start your ${device}. Close other apps using it and try again.`;
}
