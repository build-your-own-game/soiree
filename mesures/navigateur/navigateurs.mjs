// Lancer les navigateurs de la machine sans interface, et les piloter quand il le
// faut : Chrome par le protocole DevTools, Firefox par WebDriver BiDi (Firefox n'a
// plus le protocole DevTools depuis la version 129), Safari par safaridriver
// (WebDriver classique : Safari n'a pas de mode sans interface, et safaridriver
// exige que l'hôte ait coché « Autoriser l'automatisation à distance »). Node seul :
// `WebSocket` et `fetch` sont dans node depuis la version 22.

import { spawn, execFileSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { DONNEES } from "./serveur.mjs";

export const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
export const FIREFOX = "/Applications/Firefox.app/Contents/MacOS/firefox";
export const SAFARIDRIVER = "/usr/bin/safaridriver";

/** Les versions des navigateurs de la machine. */
export function versions() {
  const lire = (f) => {
    try { return f(); } catch (e) { return `inconnue (${e.message.split("\n")[0]})`; }
  };
  return {
    chrome: lire(() => execFileSync(CHROME, ["--version"]).toString().trim()),
    firefox: lire(() => execFileSync(FIREFOX, ["--version"]).toString().trim()),
    safari: lire(() => `Safari ${execFileSync("defaults", ["read", "/Applications/Safari.app/Contents/Info", "CFBundleShortVersionString"]).toString().trim()}`),
  };
}

/** Efface un profil temporaire ; les processus fils d'un navigateur y écrivent
 *  encore un instant après sa fin. */
function effacer(dossier) {
  try {
    rmSync(dossier, { recursive: true, force: true, maxRetries: 20, retryDelay: 250 });
  } catch (e) {
    console.error(`profil laissé : ${dossier} (${e.code})`);
  }
}

function profil(nom) {
  const d = join(DONNEES, "profils", `${nom}-${process.pid}-${Date.now()}`);
  mkdirSync(d, { recursive: true });
  return d;
}

/** Attend sur la sortie d'erreur d'un processus une ligne qui correspond. */
function attendreLigne(proc, motif, delai = 30000) {
  return new Promise((ok, ko) => {
    let tampon = "";
    const minuterie = setTimeout(() => ko(new Error(`rien vu de ${motif} en ${delai} ms : ${tampon.slice(-500)}`)), delai);
    const lire = (m) => {
      tampon += m.toString();
      const r = tampon.match(motif);
      if (r) { clearTimeout(minuterie); ok(r); }
    };
    proc.stderr.on("data", lire);
    proc.stdout.on("data", lire);
  });
}

/** Un client WebSocket à requêtes numérotées, pour le protocole DevTools et BiDi. */
class Client {
  constructor(ws) {
    this.ws = ws;
    this.suivant = 1;
    this.attente = new Map();
    this.ecouteurs = [];
    ws.addEventListener("message", (m) => {
      const x = JSON.parse(m.data);
      if (x.id !== undefined && this.attente.has(x.id)) {
        const { ok, ko } = this.attente.get(x.id);
        this.attente.delete(x.id);
        if (x.error) ko(new Error(typeof x.error === "string" ? `${x.error} : ${x.message}` : JSON.stringify(x.error)));
        else ok(x.result);
      } else {
        for (const e of this.ecouteurs) e(x);
      }
    });
  }

  static ouvrir(url) {
    return new Promise((ok, ko) => {
      const ws = new WebSocket(url);
      ws.addEventListener("open", () => ok(new Client(ws)));
      ws.addEventListener("error", (e) => ko(new Error(`WebSocket ${url} : ${e.message ?? e.type}`)));
    });
  }

  envoyer(method, params = {}, extra = {}) {
    const id = this.suivant++;
    return new Promise((ok, ko) => {
      this.attente.set(id, { ok, ko });
      this.ws.send(JSON.stringify({ id, method, params, ...extra }));
    });
  }

  fermer() { try { this.ws.close(); } catch { /* déjà fermé */ } }
}

/** Arrête un processus et attend sa fin (au plus 10 s, puis SIGKILL). */
function arreter(proc) {
  return new Promise((ok) => {
    if (proc.exitCode !== null) { ok(); return; }
    const dur = setTimeout(() => proc.kill("SIGKILL"), 10000);
    proc.once("exit", () => { clearTimeout(dur); ok(); });
    proc.kill("SIGTERM");
  });
}

/**
 * Chrome sans interface (`--headless=new`, le vrai navigateur sans fenêtre), un
 * profil neuf. Avec `pilote`, le protocole DevTools est ouvert.
 */
export async function ouvrirChrome(url, { pilote = false, drapeaux = [] } = {}) {
  const dossier = profil("chrome");
  const args = [
    "--headless=new",
    `--user-data-dir=${dossier}`,
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-extensions",
    "--disable-component-update",
    "--disable-sync",
    ...drapeaux,
  ];
  if (pilote) args.push("--remote-debugging-port=0");
  args.push(url);
  const proc = spawn(CHROME, args, { stdio: ["ignore", "pipe", "pipe"] });
  let client = null;
  if (pilote) {
    const [, ws] = await attendreLigne(proc, /DevTools listening on (ws:\/\/\S+)/);
    client = await Client.ouvrir(ws);
  }
  return {
    proc,
    client,
    async fermer() {
      if (client) client.fermer();
      await arreter(proc);
      effacer(dossier);
    },
  };
}

/**
 * Firefox sans interface (`--headless`), un profil neuf, sans première page ni
 * télémétrie. Avec `pilote`, WebDriver BiDi est ouvert et une session créée.
 * `preferences` s'ajoute au `user.js` du profil.
 */
export async function ouvrirFirefox(url, { pilote = false, preferences = {} } = {}) {
  const dossier = profil("firefox");
  const prefs = {
    "browser.shell.checkDefaultBrowser": false,
    "browser.startup.homepage_override.mstone": "ignore",
    "browser.aboutwelcome.enabled": false,
    "datareporting.policy.dataSubmissionEnabled": false,
    "toolkit.telemetry.reportingpolicy.firstRun": false,
    "app.update.disabledForTesting": true,
    "browser.tabs.warnOnClose": false,
    ...preferences,
  };
  writeFileSync(join(dossier, "user.js"),
    Object.entries(prefs).map(([k, v]) => `user_pref(${JSON.stringify(k)}, ${JSON.stringify(v)});`).join("\n"));
  const args = ["--headless", "--no-remote", "--profile", dossier];
  if (pilote) args.push("--remote-debugging-port", "0");
  args.push(url);
  const proc = spawn(FIREFOX, args, { stdio: ["ignore", "pipe", "pipe"] });
  let client = null;
  if (pilote) {
    const [, ws] = await attendreLigne(proc, /WebDriver BiDi listening on (ws:\/\/\S+)/);
    client = await Client.ouvrir(`${ws.replace(/\/$/, "")}/session`);
    await client.envoyer("session.new", { capabilities: {} });
  }
  return {
    proc,
    client,
    async fermer() {
      if (client) {
        await client.envoyer("session.end").catch(() => {});
        client.fermer();
      }
      await arreter(proc);
      effacer(dossier);
    },
  };
}

/**
 * Safari par safaridriver. Rend `{ refus }` si l'hôte n'a pas autorisé
 * l'automatisation à distance : le banc ne mesure alors pas Safari, et le dit.
 */
export async function ouvrirSafari(url) {
  const port = 4500 + Math.floor(Math.random() * 400);
  const proc = spawn(SAFARIDRIVER, ["-p", String(port)], { stdio: ["ignore", "pipe", "pipe"] });
  const base = `http://127.0.0.1:${port}`;
  let reponse = null;
  for (let k = 0; k < 50 && reponse === null; k++) {
    try {
      reponse = await (await fetch(`${base}/session`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ capabilities: { alwaysMatch: { browserName: "safari" } } }),
      })).json();
    } catch {
      await new Promise((f) => setTimeout(f, 200));
    }
  }
  const id = reponse?.value?.sessionId;
  if (!id) {
    await arreter(proc);
    return { refus: reponse?.value?.message ?? "safaridriver ne répond pas" };
  }
  const wd = (methode, chemin, corps) => fetch(`${base}/session/${id}${chemin}`, {
    method: methode,
    headers: { "content-type": "application/json" },
    body: corps === undefined ? undefined : JSON.stringify(corps),
  }).then((r) => r.json());
  await wd("POST", "/url", { url });
  return {
    proc,
    wd,
    async fermer() {
      await fetch(`${base}/session/${id}`, { method: "DELETE" }).catch(() => {});
      await arreter(proc);
    },
  };
}
