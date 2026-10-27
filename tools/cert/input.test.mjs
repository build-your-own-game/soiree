import test from "node:test";
import assert from "node:assert/strict";
import { launchBrowser } from "./browser.mjs";

const SURFACE = "data:text/html," + encodeURIComponent("<style>body{margin:0;user-select:none}div{width:390px;height:844px}</style><div>Native input surface</div><script>window.moves=[];window.ends=0;document.addEventListener('mousemove',e=>moves.push(e.buttons));document.addEventListener('touchend',()=>ends++);</script>");

test("interrupted native drags release the held mouse on both engines", { timeout: 30000 }, async () => {
  const browser = await launchBrowser();
  try {
    for (const engine of ["webkit", "chromium"]) {
      const tab = await browser.open(SURFACE, { engine, mobile: true });
      const input = tab.input.bind(tab);
      for (const stage of ["drag.mouse.move.start", "drag.mouse.down", "drag.mouse.move.steps"]) {
        // Fail after an actual native acknowledgement, when the driver may
        // already retain a pressed button. No synthetic DOM event is used.
        tab.input = async (operation, task) => {
          const value = await input(operation, task);
          if (operation === stage) throw new Error(`interrupted ${stage}`);
          return value;
        };
        await assert.rejects(tab.drag({ x: 20, y: 20 }, { x: 100, y: 100 }), /interrupted/);
        tab.input = input;
        await tab.page.mouse.move(30, 30);
        assert.equal(await tab.evaluate("moves.at(-1)"), 0, `${engine}: ${stage} left a pressed button`);
      }
      await tab.close();
    }
  } finally { await browser.close(); }
});

test("an interrupted Chromium touch sequence emits its native touch end", { timeout: 30000 }, async () => {
  const browser = await launchBrowser();
  try {
    const tab = await browser.open(SURFACE, { engine: "chromium", mobile: true });
    const input = tab.input.bind(tab);
    tab.input = async (operation, task) => {
      const value = await input(operation, task);
      if (operation === "swipe.cdp.touchMove") throw new Error("interrupted native touch move");
      return value;
    };
    await assert.rejects(tab.swipe([{ x: 20, y: 20 }, { x: 100, y: 100 }]), /interrupted/);
    assert.equal(await tab.evaluate("ends"), 1);
    tab.input = input;
    await tab.tap(30, 30);
    assert.equal(await tab.evaluate("ends"), 2, "the next real touch must form an independent sequence");
    await tab.close();
  } finally { await browser.close(); }
});
