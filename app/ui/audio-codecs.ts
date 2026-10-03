import workerUrl from "../node_modules/@libav.js/variant-opus-af/dist/libav-6.10.9.0-opus-af.wasm.js?url";
import wasmUrl from "../node_modules/@libav.js/variant-opus-af/dist/libav-6.10.9.0-opus-af.wasm.wasm?url";

let loading: Promise<void> | undefined;

export async function prepareAudioCodecs(): Promise<void> {
  if (globalThis.AudioEncoder && globalThis.AudioDecoder) return;

  loading ??= loadAudioCodecs().catch((error) => {
    loading = undefined;
    throw new Error("Could not load audio support", { cause: error });
  });

  await loading;
}

async function loadAudioCodecs(): Promise<void> {
  const [opus, codecs] = await Promise.all([
    import("@libav.js/variant-opus-af"),
    import("@kixelated/libavjs-webcodecs-polyfill"),
  ]);

  // LibAV normally guesses sibling filenames from its loader URL. Vite moves
  // that loader, so give it explicitly bundled worker and WASM assets instead.
  await codecs.load({
    LibAV: opus,
    polyfill: true,
    libavOptions: {
      toImport: new URL(workerUrl, location.href).href,
      wasmurl: new URL(wasmUrl, location.href).href,
      noes6: true,
    },
  });
}
