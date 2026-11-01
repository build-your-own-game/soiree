// T2 oracles: deterministic inputs, exact semantic effects and native layout.
// A successful corpus is never inferred from a lack of exceptions.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { spawn } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { freeHostPort } from './ports.mjs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

export const TEXT_CASES = Object.freeze([
  ...JSON.parse(readFileSync(new URL('./nasty.json', import.meta.url))).texts,
  'é'.repeat(500),
  '<img src=x onerror=__certTextInjected++>',
  '日本語'.repeat(30),
].map((text, index) => Object.freeze({ id: `text-${index + 1}`, text })));

export function clipUtf16(text, limit) {
  let result = '';
  for (const character of text) {
    if (result.length + character.length > limit) break;
    result += character;
  }
  return result;
}

/** Pixel-font wrapping can split a long word or replace a space with a line
 * break. It must neither drop an ordinary letter nor silently join words. */
export function wrappedTextMatches(source, drawn) {
  const expected = source.trim().replace(/\s+/gu, ' ');
  let at = 0;
  for (const character of drawn) {
    if (character === '\n') {
      if (expected[at] === ' ') at++;
    } else {
      if (!expected.startsWith(character, at)) return false;
      at += character.length;
    }
  }
  return at === expected.length;
}

export async function fillText(page, selector, text) {
  const field = page.locator(selector);
  await field.fill(text);
  const observed = await field.evaluate(element => ({ value: element.value, tag: element.tagName, max: element.maxLength }));
  const variants = observed.tag === 'TEXTAREA' ? [text.replace(/\r\n?/g, '\n')] : [text.replace(/[\r\n]/g, ''), text.replace(/\r\n?|\n/g, ' ')];
  assert.ok(variants.some(value => observed.value === value || (observed.max >= 0 && observed.value === clipUtf16(value, observed.max))),
    `native field changed input outside its declared sanitization/maxlength: ${JSON.stringify(observed)}`);
  return observed.value;
}

export function observeTextSafety() {
  window.__certTextInjected = 0;
}

// Observe event fragments, not heartbeats or the continuously held stick.
// This reads only the common postcard header (salon/core/wire.rs), after
// Welcome. Unknown/control packets fail the observation rather than proving
// that no game input was sent. No game-specific binary schema is duplicated.
export function observeTextEvents() {
  window.__certTextEvents = { parts: 0, errors: [] };
  const send = WebSocket.prototype.send;
  WebSocket.prototype.send = function (data) {
    try {
      const bytes = data instanceof ArrayBuffer ? new Uint8Array(data)
        : ArrayBuffer.isView(data) ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength) : null;
      if (!bytes) throw new Error('unexpected non-binary packet');
      let at = 0;
      const byte = () => { if (at >= bytes.length) throw new Error('short postcard header'); return bytes[at++]; };
      const number = () => { let n = 0; for (let i = 0, scale = 1; i < 10; i++, scale *= 128) { const b = byte(); n += (b & 127) * scale; if (!(b & 128)) return n; } throw new Error('invalid varint'); };
      const option = skip => { const tag = byte(); if (tag > 1) throw new Error('invalid option'); if (tag) skip(); };
      number(); number(); // epoch, sender time
      option(() => { number(); number(); }); // echo
      option(() => { throw new Error('control packet after observation baseline'); });
      option(() => { number(); number(); }); // ack
      option(number); option(number); // phase, holding
      window.__certTextEvents.parts += number(); // event fragment vector
    } catch (error) { window.__certTextEvents.errors.push(String(error)); }
    return Reflect.apply(send, this, [data]);
  };
}

// Runs inside the real browser. Range rectangles are native text layout
// rectangles, not ink/VRAM measurements. Check every clipping ancestor, even
// when an oversized child has already been clipped by overflow:hidden.
export function inspectLiteral({ selector, expected, frame = null, index = 0 }) {
  const element = document.querySelectorAll(selector)[index];
  if (!element?.isConnected) return [`missing ${selector}`];
  element.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
  const problems = [];
  if (element.textContent !== expected) problems.push(`literal text differs: ${JSON.stringify(element.textContent)} != ${JSON.stringify(expected)}`);
  if (element.children.length) problems.push('text leaf acquired HTML elements');
  if (window.__certTextInjected !== 0) problems.push('injected handler executed');
  const style = getComputedStyle(element);
  const box = element.getBoundingClientRect();
  if (style.display === 'none' || style.visibility !== 'visible' || Number(style.opacity) === 0 || (expected.replace(/[\p{Cc}\p{Cf}\p{Z}]/gu, '') && (!box.width || !box.height))) problems.push('text leaf is not visible');
  const range = document.createRange(); range.selectNodeContents(element);
  const rectangles = [...range.getClientRects()].filter(rect => rect.width && rect.height);
  if (expected.replace(/[\p{Cc}\p{Cf}\p{Z}]/gu, '') && !rectangles.length) problems.push('printable text has no layout rectangle');
  const limits = [];
  if (frame) {
    const parent = document.querySelector(frame);
    if (!parent?.contains(element)) problems.push(`missing containing frame ${frame}`);
    else limits.push({ box: parent.getBoundingClientRect(), x: true, y: true });
  }
  for (let parent = element; parent; parent = parent.parentElement) {
    const css = getComputedStyle(parent);
    if (css.display === 'none' || css.visibility !== 'visible' || Number(css.opacity) === 0) problems.push('text ancestor is not visible');
    const x = /hidden|clip/.test(css.overflowX), y = /hidden|clip/.test(css.overflowY);
    if (x || y) limits.push({ box: parent.getBoundingClientRect(), x, y });
  }
  for (const rect of rectangles) {
    for (const limit of limits) {
      if ((limit.x && (rect.left < limit.box.left - 2 || rect.right > limit.box.right + 2)) ||
          (limit.y && (rect.top < limit.box.top - 2 || rect.bottom > limit.box.bottom + 2))) problems.push('native text rectangle is clipped by its frame');
    }
  }
  return [...new Set(problems)];
}

export async function assertLiteral(page, selector, expected, frame = null) {
  return assertLiteralAt(page, selector, 0, expected, frame);
}

export async function assertLiteralAt(page, selector, index, expected, frame = null) {
  const problems = await page.evaluate(inspectLiteral, { selector, expected, frame, index });
  assert.deepEqual(problems, [], `${selector}: ${problems.join('; ')}`);
}

export async function assertRenderedText(page, expected, kind = 'bubble') {
  const result = await page.evaluate(({ expected, kind }) => {
    const projection = JSON.parse(window.__certRendered ?? 'null');
    const layout = projection?.layout;
    if (!layout || !Array.isArray(layout.texts)) return { error: 'missing submitted canvas text layout' };
    const texts = layout.texts.filter(text => text.text === expected && (!kind || text.kind === kind));
    return { viewport: layout.viewport, texts };
  }, { expected, kind });
  assert.equal(result.error, undefined, result.error);
  assert.ok(result.texts.length, `submitted canvas text missing ${kind}: ${JSON.stringify(expected)}`);
  for (const text of result.texts) {
    assert.ok([text.x, text.y, text.width, text.height].every(Number.isFinite) && text.width > 0 && text.height > 0, 'invalid submitted text bounds');
    assert.ok(text.x >= -2 && text.y >= -2 && text.x + text.width <= result.viewport.width + 2 && text.y + text.height <= result.viewport.height + 2, 'submitted text leaves canvas viewport');
  }
  return result.texts;
}

// Every game invokes these negative controls with its actual product leaf.
// Restoring the exact DOM node keeps listeners owned by the product intact.
export async function negativeTextControls(page, selector, expected, frame = null) {
  // Mutate, inspect and restore in one browser task. An authoritative view
  // can replace the model's counter every RTT; waiting for an unrelated rAF
  // or a second IPC would test a detached/replaced leaf instead of the fault.
  const result = await page.evaluate(({ selector, expected, frame, source }) => {
    const inspect = (0, eval)(`(${source})`);
    const element = document.querySelector(selector);
    if (!element) throw new Error(`missing negative-control leaf ${selector}`);
    const children = [...element.childNodes], style = element.getAttribute('style');
    let markup, clipped;
    try {
      element.innerHTML = '<b>injected markup</b>';
      markup = inspect({ selector, expected, frame });
      element.replaceChildren(...children);
      element.style.cssText += ';display:block!important;width:1px!important;height:1px!important;overflow:hidden!important;white-space:nowrap!important';
      clipped = inspect({ selector, expected, frame });
    } finally {
      element.replaceChildren(...children);
      if (style === null) element.removeAttribute('style'); else element.setAttribute('style', style);
    }
    return { markup, clipped, restored: inspect({ selector, expected, frame }) };
  }, { selector, expected, frame, source: inspectLiteral.toString() });
  assert.ok(result.markup.some(problem => /literal|HTML/.test(problem)), 'markup mutation escaped the literal oracle');
  assert.ok(result.clipped.some(problem => /clipped/.test(problem)), 'clipping mutation escaped the frame oracle');
  assert.deepEqual(result.restored, [], 'negative control failed to restore the product leaf');
}

// Isolate mutable multiplayer rooms from the parent campaign's filled room.
// Uses the release binary already built by that campaign, never recompiles.
export async function textHost({ cwd, binary, args = [], env = {}, ready }) {
  const port = await freeHostPort();
  const data = await mkdtemp(join(tmpdir(), 'soiree-text-'));
  const substitutions = value => value.replaceAll('{port}', String(port)).replaceAll('{tmp}', data);
  const child = spawn(binary, args.map(substitutions), { cwd, env: { ...process.env, ...Object.fromEntries(Object.entries(env).map(([key, value]) => [key, substitutions(value)])) }, stdio: ['ignore', 'pipe', 'pipe'] });
  let log = '';
  for (const stream of [child.stdout, child.stderr]) stream.on('data', bytes => { log = (log + bytes).slice(-16000); });
  const exited = new Promise(resolve => { child.once('exit', resolve); child.once('error', resolve); });
  const close = async () => {
    if (child.exitCode === null && child.signalCode === null && child.pid) {
      const force = setTimeout(() => child.kill('SIGKILL'), 1500);
      child.kill('SIGTERM'); try { await exited; } finally { clearTimeout(force); }
    }
    await rm(data, { recursive: true, force: true });
  };
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => finish(new Error(`text host startup: ${log}`)), 8000);
      const check = () => { if (log.includes(ready)) finish(); };
      const fail = value => finish(new Error(`text host exited ${value}: ${log}`));
      function finish(error) { clearTimeout(timer); child.stdout.off('data', check); child.stderr.off('data', check); child.off('exit', fail); child.off('error', fail); error ? reject(error) : resolve(); }
      child.stdout.on('data', check); child.stderr.on('data', check); child.once('exit', fail); child.once('error', fail); check();
    });
    return { base: `http://127.0.0.1:${port}`, close };
  } catch (error) { await close(); throw error; }
}
