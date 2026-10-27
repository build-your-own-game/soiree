// Run against a fresh test host, using the installed/shared certification harness.
import assert from "node:assert/strict";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";

const [base, harness] = process.argv.slice(2);
if (!base || !harness) throw new Error("Usage: node tools/press-check.mjs URL HARNESS_DIRECTORY");
const { launchBrowser } = await import(pathToFileURL(join(resolve(harness), "browser.mjs")));
const browser = await launchBrowser();
try {
  const tv = await browser.open(base);
  const phone = new URL("/play?salon=ws", base).href;
  const first = await browser.open(phone, { mobile: true });
  const peer = await browser.open(phone, { mobile: true });
  await first.page.locator("#tap").waitFor({ timeout: 3000 });
  await peer.page.locator("#tap").waitFor({ timeout: 3000 });
  const total = async () => Number.parseInt(await tv.page.locator("h1").innerText({ timeout: 3000 }), 10);
  const before = await total();
  await first.page.evaluate(() => { window.originalTap = document.querySelector("#tap"); });
  const bounds = await first.page.locator("#tap").boundingBox();
  assert.ok(bounds);
  await first.page.mouse.move(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
  await first.page.mouse.down();
  // Programmatic peer activation avoids switching native mouse focus away
  // from the finger being held; it follows the peer's actual click handler.
  await peer.page.locator("#tap").evaluate((button) => button.click());
  await tv.page.waitForFunction((value) => Number.parseInt(document.querySelector("h1").textContent, 10) === value + 1, before, { timeout: 3000 });
  assert.equal(await first.page.evaluate(() => window.originalTap === document.querySelector("#tap")), true);
  await first.page.mouse.up();
  await tv.page.waitForFunction((value) => Number.parseInt(document.querySelector("h1").textContent, 10) === value + 2, before, { timeout: 3000 });
  assert.equal(await total(), before + 2);
  console.log("Held press preserved during a peer's view update: two actions, two points.");
} finally { await browser.close(); }
