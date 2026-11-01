// This starter exposes no arbitrary text entry. Exercise every real text
// source: names, personal/global counters and button/status labels.
import assert from 'node:assert/strict';
const moduleUrl = process.env.SOIREE_CERT_BROWSER_MODULE;
if (!moduleUrl) throw new Error('Use soiree certify -- --only text');
const { launchBrowser } = await import(moduleUrl);
const { assertLiteral, negativeTextControls, observeTextSafety } = await import(new URL('./text.mjs', moduleUrl));
const base = process.argv[2] ?? process.env.SOIREE_CERT_URL;
const browser = await launchBrowser();
try {
  for (const engine of ['chromium', 'webkit']) {
    const tv = await browser.open(null, { engine });
    const phone = await browser.open(null, { engine, mobile: true });
    try {
      await tv.context.addInitScript(observeTextSafety);
      await phone.context.addInitScript(observeTextSafety);
      await tv.page.goto(new URL('/?salon=ws', base).href);
      await phone.page.goto(new URL(`/play?salon=ws&seat=text-${engine}`, base).href);
      await phone.page.locator('#tap:not([hidden])').waitFor();
      const initial = Number((await tv.page.locator('h1').textContent()).match(/^(\d+) taps$/)?.[1]);
      const personal = Number((await phone.page.locator('#view p').textContent()).match(/^Toi : (\d+) · Tous : \d+$/)?.[1]);
      assert.ok(Number.isSafeInteger(initial) && Number.isSafeInteger(personal));
      const names = await tv.page.locator('#view li').allTextContents();
      assert.ok(names.length > 0 && names.every(name => /^.+ : \d+( · \d+ ms)?$/.test(name)), 'missing real player names/counters');
      for (let i = 1; i <= 23; i++) {
        await phone.page.locator('#tap').click();
        await tv.page.waitForFunction(expected => document.querySelector('h1')?.textContent === expected, `${initial + i} taps`);
        await phone.page.waitForFunction(expected => document.querySelector('#view p')?.textContent === expected, `Toi : ${personal + i} · Tous : ${initial + i}`);
        await assertLiteral(tv.page, 'h1', `${initial + i} taps`, '#view');
        await assertLiteral(phone.page, '#view p', `Toi : ${personal + i} · Tous : ${initial + i}`, '#view');
      }
      const leaves = await tv.page.locator('#view li').allTextContents();
      for (let i = 0; i < leaves.length; i++) await assertLiteral(tv.page, `#view li:nth-child(${i + 1})`, leaves[i], '#view');
      const button = await phone.page.locator('#tap').textContent();
      assert.ok(button.trim()); await assertLiteral(phone.page, '#tap', button);
      await negativeTextControls(tv.page, 'h1', `${initial + 23} taps`, '#view');
      assert.deepEqual(tv.errors, []); assert.deepEqual(phone.errors, []);
      console.log(`${engine}: no free-text entry; names, button and 23 exact authoritative personal/global counter updates fit real frames; HTML/clipping negative controls passed`);
    } finally { await phone.context.close(); await tv.context.close(); }
  }
} finally { await browser.close(); }
