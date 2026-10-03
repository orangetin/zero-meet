import { expect, test } from "@playwright/test";

test("track metadata requests preserve an active audio subscription", async ({ page }) => {
  await page.goto("/tests/fixture.html");

  const result = await page.evaluate(async () => {
    // Vite resolves the package's browser imports here, including its worklets.
    // @ts-expect-error This is a browser URL, not a filesystem import.
    const { Broadcast } = await import("/node_modules/@moq/publish/index.js");
    const broadcast = new Broadcast({
      enabled: true,
      name: "regression.hang",
      connection: { publish() {} },
    });
    const audio = broadcast.audio("audio");
    const settle = () => new Promise((resolve) => setTimeout(resolve, 20));
    await settle();

    const network = broadcast.net.peek();
    await network.track("audio").info();
    await settle();
    const metadataStartedEncoding = !!audio.track.peek();

    const subscriber = network.subscribe("audio");
    await subscriber.info();
    await settle();
    const original = audio.track.peek();

    await network.track("audio").info();
    await settle();
    const preserved = audio.track.peek() === original && original.closed.peek() === undefined;

    subscriber.close();
    audio.close();
    broadcast.close();

    return { metadataStartedEncoding, preserved };
  });

  expect(result).toEqual({ metadataStartedEncoding: false, preserved: true });
});
