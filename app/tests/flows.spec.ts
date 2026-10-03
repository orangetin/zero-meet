import { test, expect } from "@playwright/test";

test("opens invitation from launch", async ({ page }) => {
  const invite = "zero-meet://test-room";
  await page.goto(`/tests/fixture.html?invite=${encodeURIComponent(invite)}`);
  await expect(page.getByLabel("Your name")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Join room", exact: true }),
  ).toBeVisible();
});

test("create, permission denial, receive-only join, paging, pinning and leave", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/tests/fixture.html");
  await expect(
    page.getByRole("heading", { name: "Start or join a call" }),
  ).toBeVisible();
  await page.screenshot({ path: "../artifacts/home.png" });
  await page.getByLabel("Your meeting server").fill("test-server");
  await page.getByRole("button", { name: "Create a room" }).click();
  await expect(page.getByRole("alert")).toContainText("Allow camera access");
  await page.getByLabel("Your name").fill("Taylor");
  await page.screenshot({ path: "../artifacts/preview.png" });
  await page.getByRole("button", { name: "Join room", exact: true }).click();
  await expect(page.locator(".participant-grid article")).toHaveCount(9);
  await expect(
    page.getByRole("heading", { name: "Finding our way back…" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Next participants" }).click();
  await expect(page.locator(".participant-grid")).toContainText("Guest 10");
  await expect(page.locator(".participant-grid")).not.toContainText("Taylor");
  await page.getByRole("button", { name: "Pin Guest 10", exact: true }).click();
  await expect(page.locator(".participant-grid article")).toHaveCount(9);
  await expect(page.locator("article.featured")).toContainText("Guest 10");
  await page.screenshot({ path: "../artifacts/call.png" });

  const leaveButton = page.getByRole("button", { name: "Leave", exact: true });
  const leaveDialog = page.getByRole("dialog", { name: "Leave meeting?" });

  await leaveButton.click();
  await expect(leaveDialog).toBeVisible();
  await expect(
    leaveDialog.getByRole("button", { name: "Cancel" }),
  ).toBeFocused();
  await leaveDialog.getByRole("button", { name: "Cancel" }).click();
  await expect(leaveDialog).not.toBeVisible();
  await expect(leaveButton).toBeFocused();
  await expect(page.locator(".participant-grid article")).toHaveCount(9);

  await leaveButton.click();
  await page.keyboard.press("Escape");
  await expect(leaveDialog).not.toBeVisible();
  await expect(leaveButton).toBeFocused();

  await page.getByRole("button", { name: "Leave", exact: true }).click();
  await leaveDialog.getByRole("button", { name: "Leave", exact: true }).click();
  await expect(page.getByLabel("Your meeting server")).toHaveValue(
    "test-server",
  );
  expect(errors).toEqual([]);
});

test("invalid invitations leave the user on home", async ({ page }) => {
  await page.goto("/tests/fixture.html");
  await page.getByLabel("Invitation link").fill("bad");
  await page.getByRole("button", { name: "Open invitation" }).click();
  await expect(page.getByRole("alert")).toContainText("invalid");
  await expect(
    page.getByRole("button", { name: "Create a room" }),
  ).toBeVisible();
});
