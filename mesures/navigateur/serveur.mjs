// Le serveur du banc : sert les pages (`page/`) et les données de la capture
// (`soiree/target/mesures-navigateur/`), reçoit les résultats et le journal des
// pages. Il pose COOP et COEP : une page isolée (`crossOriginIsolated`) reçoit
// l'horloge la plus fine du navigateur (mesurée le 2026-11-05 : 5 µs dans Chrome 156,
// 20 µs dans Firefox 158), et c'est la seule façon d'approcher la durée d'un appel.
// Node seul, sans dépendance.

import { createServer } from "node:http";
import { mkdirSync, readFileSync, writeFileSync, existsSync, statSync } from "node:fs";
import { dirname, extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const ICI = dirname(fileURLToPath(import.meta.url));
export const DONNEES = resolve(ICI, "../../target/mesures-navigateur");

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".json": "application/json",
  ".wasm": "application/wasm",
  ".bin": "application/octet-stream",
};

function fichier(racine, chemin) {
  const p = normalize(join(racine, decodeURIComponent(chemin)));
  if (!p.startsWith(racine)) return null;
  if (!existsSync(p) || !statSync(p).isFile()) return null;
  return p;
}

function corps(req) {
  return new Promise((ok, ko) => {
    const morceaux = [];
    req.on("data", (m) => morceaux.push(m));
    req.on("end", () => ok(Buffer.concat(morceaux)));
    req.on("error", ko);
  });
}

/**
 * Démarre le serveur sur 127.0.0.1. `surResultat(sorte, params, json)` est appelé
 * pour chaque résultat reçu (`/resultat`, `/cadence`).
 */
export function demarrerServeur({ port = 0, surResultat = () => {}, surJournal = () => {} } = {}) {
  const pages = join(ICI, "page");
  const serveur = createServer(async (req, res) => {
    const url = new URL(req.url, "http://127.0.0.1");
    res.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    res.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
    res.setHeader("Cross-Origin-Resource-Policy", "same-origin");
    res.setHeader("Cache-Control", "no-store");
    try {
      if (req.method === "POST") {
        const b = await corps(req);
        const params = Object.fromEntries(url.searchParams);
        if (url.pathname === "/journal") {
          surJournal(params, b.toString("utf8"));
        } else if (url.pathname === "/resultat" || url.pathname === "/cadence") {
          surResultat(url.pathname.slice(1), params, JSON.parse(b.toString("utf8")));
        } else {
          res.writeHead(404).end();
          return;
        }
        res.writeHead(204).end();
        return;
      }
      let p = null;
      if (url.pathname.startsWith("/page/")) p = fichier(pages, url.pathname.slice(6));
      else if (url.pathname.startsWith("/donnees/")) p = fichier(DONNEES, url.pathname.slice(9));
      else if (url.pathname === "/favicon.ico") { res.writeHead(204).end(); return; }
      if (!p) { res.writeHead(404).end(); return; }
      res.writeHead(200, { "content-type": TYPES[extname(p)] ?? "application/octet-stream" });
      res.end(readFileSync(p));
    } catch (e) {
      res.writeHead(500).end(String(e));
    }
  });
  return new Promise((ok) => {
    serveur.listen(port, "127.0.0.1", () => {
      const adresse = `http://127.0.0.1:${serveur.address().port}`;
      // Les navigateurs gardent des connexions ouvertes : on les coupe, sans quoi
      // `close` attend qu'elles expirent.
      ok({ adresse, fermer: () => new Promise((f) => { serveur.close(f); serveur.closeAllConnections(); }) });
    });
  });
}

/** Écrit un JSON, en créant son dossier. */
export function ecrireJson(chemin, valeur) {
  mkdirSync(dirname(chemin), { recursive: true });
  writeFileSync(chemin, JSON.stringify(valeur));
}
