import test from "node:test";
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import http from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execute = promisify(execFile);
const example = fileURLToPath(new URL("../../modele/tools/progression-exemple.mjs", import.meta.url));

/** A TV with rounds, as a finite game shows them. `ends` removes its last screen. */
function tvPage(ends) {
  const screens = ends
    ? ["<p class=accueil>accueil</p>", "<p class=manche>manche 1</p>", "<p class=manche>manche 2</p>", "<p class=fin>fin</p>"]
    : ["<p class=accueil>accueil</p>", "<p class=manche>manche 1</p>", "<p class=manche>manche 2</p>"];
  return `<!doctype html><title>tv</title><main id=view></main><script>
const screens = ${JSON.stringify(screens)};
let at = 0;
const show = () => { document.querySelector("#view").innerHTML = screens[at]; };
addEventListener("keydown", (event) => {
  if (event.key === "Enter" && at === 0) at = 1;
  else if (event.key === " ") at = (at + 1) % screens.length || ${ends ? "0" : "1"};
  show();
});
show();
</script>`;
}

const phonePage = `<!doctype html><title>phone</title><input><button>Entrer</button><script>
document.querySelector("button").onclick = () => { document.body.innerHTML = "<p id=seated>assis</p>"; };
</script>`;

async function run(ends) {
  const root = mkdtempSync(join(tmpdir(), "soiree-progression-example-"));
  const server = http.createServer((request, response) => {
    response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    response.end(request.url.startsWith("/play") ? phonePage : tvPage(ends));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    mkdirSync(join(root, "tools"));
    copyFileSync(example, join(root, "tools/progression-check.mjs"));
    writeFileSync(join(root, "cert.json"), JSON.stringify({
      tv: "/", phone: "/play", players: { min: 2 },
      join: [{ fill: "input", text: "{name}" }, { click: "button" }],
      joined: "document.querySelector('#seated') !== null",
      start: { key: "Enter" },
      advance: { key: " ", progress: "document.querySelector('#view').textContent" },
    }));
    const url = `http://127.0.0.1:${server.address().port}/`;
    return await execute(process.execPath, ["tools/progression-check.mjs", url], {
      cwd: root, timeout: 60000,
      env: { ...process.env, SOIREE_CERT_BROWSER_MODULE: new URL("./browser.mjs", import.meta.url).href },
    });
  } finally {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
    rmSync(root, { recursive: true, force: true });
  }
}

test("the example a finite game copies reaches a real end, then a new game", { timeout: 90000 }, async () => {
  const { stdout } = await run(true);
  assert.match(stdout, /D2 : fin réelle après 2 avancées/);
});

test("the same example fails on a game that only moves between its rounds", { timeout: 90000 }, async () => {
  await assert.rejects(run(false), (error) => {
    assert.match(error.stderr, /without a terminal/);
    return true;
  });
});
