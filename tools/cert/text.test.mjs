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

test('event observation separates state/heartbeat from events and preserves decode failures', () => {
  const context = vm.createContext({});
  vm.runInContext(`globalThis.window=globalThis;globalThis.sent=[];globalThis.WebSocket=class {send(bytes){sent.push(bytes)}};(${observeTextEvents.toString()})();`, context);
  const send = bytes => vm.runInContext(`new WebSocket().send(new Uint8Array(${JSON.stringify(bytes)}));JSON.stringify(__certTextEvents)`, context);
  assert.deepEqual(JSON.parse(send([1,0,0,0,0,0,0,0,0])), { parts: 0, errors: [] });
  // One actual event, with the remainder a single Chat("A") fragment.
  assert.equal(JSON.parse(send([1,0,0,0,0,0,0,1,0,0,1,0,0,3,3,1,65,0])).parts, 1);
  assert.equal(JSON.parse(send([1,0,0,0,0,0,0,0,0])).parts, 1);
  assert.ok(JSON.parse(send([1,0,0,1])).errors.length > 0, 'unknown control cannot prove no event was sent');
  assert.equal(vm.runInContext('sent.length', context), 4, 'observer changed transport delivery');
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
