// D2 : une vraie partie va jusqu'à sa fin, puis une nouvelle reprend, sur le même
// hôte et les mêmes téléphones. Ce modèle n'a pas de fin : rien ne lance ce
// fichier ici. Un jeu fini le copie dans ses `tools/` et le déclare dans
// `cert.json.progression.checks` (.soiree/CERTIFICATION.md). `cert.json` donne
// `start`, `advance`, `join`, `joined` et `players` ; adapte les deux expressions
// ci-dessous et, si besoin, `restart`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

// Vraie uniquement sur la TV de fin de partie, jamais pendant une manche.
const FINISHED = "document.querySelector('.fin') !== null";
// Vraie quand la partie d'après tourne de nouveau, pas seulement quand l'écran change.
const RUNNING_AGAIN = "document.querySelector('.manche') !== null";

const moduleUrl = process.env.SOIREE_CERT_BROWSER_MODULE;
if (!moduleUrl) throw new Error("Use soiree certify -- --only progression");
const { launchBrowser } = await import(moduleUrl);
const { AdvanceCycle, advanceOnce } = await import(new URL("./progression.mjs", moduleUrl));
const { verifyTerminalJourney } = await import(new URL("./progression-verification.mjs", moduleUrl));

const cert = JSON.parse(readFileSync(new URL("../cert.json", import.meta.url)));
const base = new URL(process.argv[2] ?? process.env.SOIREE_CERT_URL).origin;
const press = (tab, action) => (action.text ? tab.page.getByText(action.text).click() : tab.key(action.key));

const browser = await launchBrowser({ webkitPhones: false });
try {
  const tv = await browser.open(`${base}${cert.tv}`);
  for (let seat = 1; seat <= (cert.players?.min ?? 1); seat++) {
    const phone = await browser.open(`${base}${cert.phone}?salon=ws&seat=d2-${seat}`, { mobile: true });
    for (const step of cert.join ?? []) {
      if (step.fill) await phone.page.locator(step.fill).fill(step.text.replace("{name}", `D2-${seat}`));
      if (step.click) await phone.page.locator(step.click).click();
      if (step.press) await phone.key(step.press);
    }
    await phone.page.waitForFunction(cert.joined);
  }
  if (cert.start) await press(tv, cert.start);

  const read = async () => ({
    progress: await tv.page.evaluate((expression) => (0, eval)(expression), cert.advance.progress),
    finished: await tv.page.evaluate(FINISHED),
    running: await tv.page.evaluate(RUNNING_AGAIN),
  });
  const cycle = new AdvanceCycle();
  const advance = () => advanceOnce(tv, cert.advance, cycle, press);
  const journey = await verifyTerminalJourney({
    read,
    advance,
    terminal: (state) => state.finished,
    // Comment ton jeu repart depuis l'écran de fin : ici, une avancée puis `start`.
    restart: async () => {
      await advance();
      if (cert.start) await press(tv, cert.start);
    },
    restarted: (state) => state.running,
    maxSteps: 128,
    timeoutMs: 60_000,
  });

  // Un écran qui change ne suffit pas : le même observateur refuse une fin jamais vue.
  await assert.rejects(
    verifyTerminalJourney({ read, advance, terminal: () => false, restart: async () => {}, restarted: () => true, maxSteps: 3, timeoutMs: 12_000 }),
    /without a terminal/,
  );
  assert.deepEqual(tv.errors, []);
  console.log(`D2 : fin réelle après ${journey.steps} avancées, puis une nouvelle partie`);
} finally {
  await browser.close();
}
