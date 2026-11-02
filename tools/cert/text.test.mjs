import test from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import { launchBrowser } from './browser.mjs';
import { TEXT_CASES, clipUtf16, assertLiteral, negativeTextControls, observeTextSafety, observeTextEvents, wrappedTextMatches } from './text.mjs';

test('corpus includes distinct dangerous/Unicode/empty/over-limit inputs', () => {
  assert.ok(TEXT_CASES.length >= 23);
  assert.ok(TEXT_CASES.some(test => test.text.includes('onerror=')));
  assert.ok(TEXT_CASES.some(test => test.text === '\0'));
  assert.ok(TEXT_CASES.some(test => test.text === ''));
  assert.equal(clipUtf16('a🎲b', 2), 'a');
  assert.equal(clipUtf16('a🎲b', 3), 'a🎲');
});

test('font reflow preserves letters and actual word separation', () => {
  assert.equal(wrappedTextMatches('one two', 'one\ntwo'), true);
  assert.equal(wrappedTextMatches('longword', 'long\nword'), true);
  assert.equal(wrappedTextMatches('one two', 'onetwo'), false);
  assert.equal(wrappedTextMatches('longword', 'long\nwor'), false);
  assert.equal(wrappedTextMatches('<b>X</b>', 'X'), false);
  assert.equal(wrappedTextMatches('a 🎉', 'a\n🎉'), true);
});

// The same bytes as `an_ack_reads_as_the_harness_reads_it` in salon's wire.rs.
const ACK_WITH_PARTS = [1,2,0,0,1,5,0,1,1,5,3,1,1,0,0,0,0,0];

test('event observation separates state/heartbeat from events and preserves decode failures', () => {
  const context = vm.createContext({});
  vm.runInContext(`globalThis.window=globalThis;globalThis.sent=[];globalThis.WebSocket=class {send(bytes){sent.push(bytes)}};(${observeTextEvents.toString()})();`, context);
  const send = bytes => vm.runInContext(`new WebSocket().send(new Uint8Array(${JSON.stringify(bytes)}));JSON.stringify(__certTextEvents)`, context);
  assert.deepEqual(JSON.parse(send([1,0,0,0,0,0,0,0,0])), { parts: 0, errors: [] });
  // One actual event, with the remainder a single Chat("A") fragment.
  assert.equal(JSON.parse(send([1,0,0,0,0,0,0,1,0,0,1,0,0,3,3,1,65,0])).parts, 1);
  assert.equal(JSON.parse(send([1,0,0,0,0,0,0,0,0])).parts, 1);
  // An ack with its fragments: what lies past it still reads as no event.
  assert.deepEqual(JSON.parse(send(ACK_WITH_PARTS)), { parts: 1, errors: [] });
  assert.ok(JSON.parse(send([1,0,0,1])).errors.length > 0, 'unknown control cannot prove no event was sent');
  assert.equal(vm.runInContext('sent.length', context), 5, 'observer changed transport delivery');
});

test('native literal/bounds oracles reject markup, clipping and missing leaves in both engines', async () => {
  const browser = await launchBrowser();
  try {
    for (const engine of ['chromium', 'webkit']) {
      const tab = await browser.open(null, { engine });
      try {
        await tab.page.addInitScript(observeTextSafety);
        await tab.page.goto('data:text/html,' + encodeURIComponent('<style>#text{display:block;white-space:pre-wrap;overflow-wrap:anywhere}</style><div id="frame" style="width:600px"><span id="text"></span></div>'));
        await tab.page.evaluate(observeTextSafety);
        for (const { text } of TEXT_CASES.filter(test => test.text.trim())) {
          await tab.page.locator('#text').evaluate((element, value) => element.textContent = value, text);
          await assertLiteral(tab.page, '#text', text, '#frame');
        }
        await tab.page.locator('#text').evaluate(element => element.textContent = 'Product text');
        await negativeTextControls(tab.page, '#text', 'Product text', '#frame');
        await assert.rejects(assertLiteral(tab.page, '#missing', 'Product text'), /missing|No node/);
      } finally { await tab.context.close(); }
    }
  } finally { await browser.close(); }
});

test('a literal is read at rest: an entrance waits, what stays hidden still fails', async () => {
  const browser = await launchBrowser();
  const entrance = '@keyframes pop{0%{opacity:0}100%{opacity:1}}.pop{animation:pop .4s ease-out both}';
  try {
    for (const engine of ['chromium', 'webkit']) {
      const tab = await browser.open(null, { engine });
      try {
        await tab.page.goto('data:text/html,' + encodeURIComponent(`<style>${entrance}@keyframes gone{to{opacity:0}}.gone{animation:gone .1s forwards}@keyframes sway{to{transform:rotate(2deg)}}.sway{animation:sway 1s infinite}</style><div id="frame"><span id="text"></span></div>`));
        await tab.page.evaluate(observeTextSafety);
        const show = (html) => tab.page.evaluate(html => { document.querySelector('#frame').innerHTML = html; }, html);
        // The ancestor's first frame has opacity 0: the state the product is in
        // when the view that brought the text has just been drawn.
        await show('<div class="pop"><span id="text">Arrives</span></div>');
        await assertLiteral(tab.page, '#text', 'Arrives', '#frame');
        // A wobble never ends, and is no reason to wait for one.
        await show('<div class="sway"><span id="text">Sways</span></div>');
        await assertLiteral(tab.page, '#text', 'Sways', '#frame');
        for (const [html, why] of [
          ['<div style="opacity:0"><span id="text">Hidden</span></div>', 'stays at opacity 0'],
          ['<div class="gone"><span id="text">Hidden</span></div>', 'ends at opacity 0'],
          ['<div class="pop" style="animation-play-state:paused"><span id="text">Hidden</span></div>', 'never gets past its first frame'],
        ]) {
          await show(html);
          await assert.rejects(assertLiteral(tab.page, '#text', 'Hidden', '#frame'), /not visible/, why);
        }
      } finally { await tab.context.close(); }
    }
  } finally { await browser.close(); }
});
