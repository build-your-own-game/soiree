import test from 'node:test';
import assert from 'node:assert/strict';
import { launchBrowser } from './browser.mjs';
import { refusalMessage, phoneFieldSizes, fieldSizeFailures, seatAfterReturn } from './presentation.mjs';

test('a returning page keeps its seat only when token and device are both seen again', () => {
  const before = { token: 'a', device: 3 };
  const after = { token: 'a', device: 3, welcomed: true };
  assert.equal(seatAfterReturn(before, after), 'same');
  assert.equal(seatAfterReturn(before, { ...after, device: 4 }), 'other');
  assert.equal(seatAfterReturn(before, { ...after, token: 'b' }), 'other');
  assert.equal(seatAfterReturn(before, { ...after, welcomed: false }), 'unknown');
  assert.equal(seatAfterReturn({ token: null, device: 3 }, after), 'unknown');
  assert.equal(seatAfterReturn(before, { ...after, device: null }), 'unknown');
});

test('refusal requires the declared visible message, not an empty or disconnected page', async () => {
  const browser = await launchBrowser();
  try {
    for (const engine of ['chromium', 'webkit']) {
      const tab = await browser.open(null, { engine });
      try {
        const expected = { selector: '[role=alert]', text: 'Every place is taken.' };
        await tab.page.setContent('<p role="alert">Every place is taken. Try again.</p>');
        assert.match((await refusalMessage(tab.page, expected)).text, /Try again/);
        for (const html of ['<p role="alert">Connecting…</p>', '<p role="alert" hidden>Every place is taken.</p>',
          '<p role="alert" style="position:absolute;left:99999px">Every place is taken.</p>']) {
          await tab.page.setContent(html);
          await assert.rejects(refusalMessage(tab.page, expected));
        }
      } finally { await tab.close(); }
    }
  } finally { await browser.close(); }
});

test('visible input font sizes include contenteditable and reject a 15px field', async () => {
  const browser = await launchBrowser({ webkitPhones: false });
  try {
    const tab = await browser.open(null);
    await tab.page.setContent('<input id="small" style="font-size:15px"><textarea style="font-size:16px"></textarea>'
      + '<span contenteditable="plaintext-only" style="font-size:17px">label</span><input hidden style="font-size:1px">');
    const fields = await tab.page.evaluate(phoneFieldSizes);
    assert.equal(fields.length, 3);
    assert.deepEqual(fieldSizeFailures([{ label: 'phone', fields }]), ['phone input#small : 15 px']);
    assert.equal(fieldSizeFailures([{ label: 'phone', fields: [{ tag: 'input', id: '', font_px: NaN }] }]).length, 1);
  } finally { await browser.close(); }
});
