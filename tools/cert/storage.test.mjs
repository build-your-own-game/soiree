import test from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { once } from "node:events";
import { launchBrowser } from "./browser.mjs";

const KEY = "salon.token.Phone.scope";
const BLOCK = `for (const name of ["localStorage", "sessionStorage"]) Object.defineProperty(window, name, { get() { throw new DOMException("refusé", "SecurityError"); } });`;

test("fixture identity is restored before application boot after forbidden storage", { timeout: 30000 }, async () => {
  const server = createServer((_, response) => {
    response.setHeader("content-type", "text/html");
    response.end(`<script>try { window.bootToken = localStorage.getItem(${JSON.stringify(KEY)}); } catch { window.bootToken = "denied"; }</script>Ready`);
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  let browser;
  try {
    browser = await launchBrowser();
    for (const engine of ["webkit", "chromium"]) {
      const tab = await browser.open(`http://127.0.0.1:${server.address().port}`, { engine, mobile: true });
      await tab.evaluate(`localStorage.setItem(${JSON.stringify(KEY)}, "old-identity")`);
      const allow = await tab.beforeScripts(BLOCK);
      await tab.reload();
      assert.equal(await tab.evaluate("window.bootToken"), "denied");
      await allow();
      // Removing the initializer does not repair its current document.
      await assert.rejects(tab.evaluate(`localStorage.setItem(${JSON.stringify(KEY)}, "new-identity")`), /refusé/);
      await tab.reloadWithStorage([[KEY, "new-identity"]]);
      assert.equal(await tab.evaluate("window.bootToken"), "new-identity", `${engine}: application read the wrong identity`);
      assert.equal(await tab.evaluate(`localStorage.getItem(${JSON.stringify(KEY)})`), "new-identity");
      // The one-use initializer must not overwrite a later legitimate save.
      await tab.evaluate(`localStorage.setItem(${JSON.stringify(KEY)}, "later-identity")`);
      await tab.reload();
      assert.equal(await tab.evaluate("window.bootToken"), "later-identity");
      await tab.close();
    }
  } finally {
    await browser?.close();
    await new Promise((resolve) => server.close(resolve));
  }
});
