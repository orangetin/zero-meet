import { expect, test } from "@playwright/test";

test("Opus fallback encodes and decodes without native audio codecs", async ({
  page,
}) => {
  await page.addInitScript(() => {
    for (const name of [
      "AudioEncoder",
      "AudioDecoder",
      "AudioData",
      "EncodedAudioChunk",
    ]) {
      Reflect.deleteProperty(globalThis, name);
    }
  });
  await page.goto("/tests/fixture.html");

  const result = await page.evaluate(async () => {
    // @ts-expect-error Vite serves this browser module from the UI source tree.
    const { prepareAudioCodecs } = await import("/ui/audio-codecs.ts");
    const videoEncoder = globalThis.VideoEncoder;
    await Promise.all([prepareAudioCodecs(), prepareAudioCodecs()]);

    const chunks: EncodedAudioChunk[] = [];
    const errors: string[] = [];
    let decodedFrames = 0;
    let peak = 0;
    const config = { codec: "opus", sampleRate: 48_000, numberOfChannels: 1 };
    const encoder = new AudioEncoder({
      output: (chunk) => chunks.push(chunk),
      error: (error) => errors.push(String(error)),
    });
    const decoder = new AudioDecoder({
      output: (frame) => {
        const samples = new Float32Array(frame.numberOfFrames);
        frame.copyTo(samples, {
          planeIndex: 0,
          format: "f32-planar",
          frameOffset: 0,
          frameCount: frame.numberOfFrames,
        });
        for (const sample of samples) peak = Math.max(peak, Math.abs(sample));
        decodedFrames += frame.numberOfFrames;
        frame.close();
      },
      error: (error) => errors.push(String(error)),
    });

    try {
      encoder.configure({ ...config, bitrate: 32_000 });
      decoder.configure(config);

      for (let index = 0; index < 5; index++) {
        const samples = Float32Array.from(
          { length: 960 },
          (_, sample) =>
            0.25 *
            Math.sin((2 * Math.PI * 440 * (index * 960 + sample)) / 48_000),
        );
        const frame = new AudioData({
          format: "f32-planar",
          sampleRate: 48_000,
          numberOfChannels: 1,
          numberOfFrames: samples.length,
          timestamp: index * 20_000,
          data: samples,
        });
        encoder.encode(frame);
        frame.close();
      }

      await encoder.flush();
      for (const chunk of chunks) decoder.decode(chunk);
      await decoder.flush();

      return {
        chunks: chunks.length,
        decodedFrames,
        peak,
        errors,
        videoUnchanged: videoEncoder === globalThis.VideoEncoder,
      };
    } finally {
      encoder.close();
      decoder.close();
    }
  });

  expect(result.errors).toEqual([]);
  expect(result.chunks).toBeGreaterThan(0);
  expect(result.decodedFrames).toBeGreaterThan(0);
  expect(result.peak).toBeGreaterThan(0.1);
  expect(result.videoUnchanged).toBe(true);
});
