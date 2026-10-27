# Changements

Chaque version a sa section, de la plus récente à la plus ancienne. « Passer à cette version » s'adresse à l'agent d'un jeu (voir `AGENTS.md`).

## v0.1.0 — 2026-10-27

La première version : salon, la télécommande iPhone, les règles communes, le harnais de certification, le catalogue d'items par mécanique, l'outil `soiree` et le modèle de jeu.

### Passer à cette version

1. Installe l'outil : `cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.1.0 soiree`.
2. Un jeu neuf : `soiree new <nom>`. Un jeu existant : ajoute `soiree.toml` (`version = "0.1.0"`), remplace les chemins vers salon par `{ git = "https://github.com/build-your-own-game/soiree", tag = "v0.1.0" }`, puis `soiree check`.
3. Les règles se sont durcies depuis `v0.1.0-pre.1`. Recopie dans le jeu les lints de `outil/regles/cargo.toml` (à partir de `wildcard_enum_match_arm`, en retirant `cast_precision_loss = "allow"`) et `max-fn-params-bools = 0` de `outil/regles/clippy.toml` ; `soiree check` dit ce qui manque. Puis `cargo clippy --all-targets` et `cargo clippy --target wasm32-unknown-unknown` disent quoi corriger. Les idiomes attendus :
   - `wildcard_enum_match_arm` : nomme les variants du bras de repli (`A | B(_) => {}`), ou extrais avec `if let`. Un `#[expect]` pour une enum étrangère large, ou pour un `other => panic!` dans un test.
   - `fn_params_excessive_bools` (`max-fn-params-bools = 0`) : une enum à deux variants, ou deux méthodes. Un `#[expect]` quand le bool est la donnée elle-même (une touche enfoncée, un prédicat).
   - `cast_precision_loss` : `f64::from(u32)` ; `Ms::as_f64()` pour le temps de salon ; sinon une seule fonction de conversion avec son `#[expect]`.
   - `arithmetic_side_effects` : `saturating_*` pour le temps et les statistiques, `checked_*` et une erreur pour un identifiant ou une entrée, `wrapping_*` pour un hachage ou une animation, itérateurs ou `checked_sub(1)` pour les index, un `NonZero*` comme diviseur. Une crate de rendu, et les tests, peuvent s'en exempter en tête de crate ou de module : `#![expect(clippy::arithmetic_side_effects, reason = "…")]`.
   - `let_underscore_untyped` et `unused_result_ok` : `let _: Result<(), JsValue> = element.focus();`, jamais `.ok();`.
4. Depuis `v0.1.0-pre.1`, pour suivre ces règles, quelques API de salon et de la télécommande ont changé :
   - salon-core : `Host::set_open(b)` devient `Host::open()` ou `Host::close()`, et `Host::local_present(now, device, b)` devient `local_plugged(now, device)` ou `local_unplugged(now, device)` ;
   - salon-host : `Clock::at` renvoie un `Option<Instant>` (`None` au-delà de ce qu'`Instant` peut tenir) ;
   - salon-host : `qr_svg(url, dark, light)`, qui donnait du SVG en texte, devient `qr(url) -> Result<Qr, String>` : les modules du QR code, dans le type `salon_core::Qr`. La page le dessine avec `salon_web::qr(&qr)`, un `<svg>` dont les couleurs viennent du CSS du jeu (`fill` sur `path`, `background` sur `svg`) ;
   - remote-core : `landscape_left: bool` devient `Orientation::LandscapeLeft` ou `Orientation::LandscapeRight` dans `Wheel::update`, `Pointer::motion` et `remote::Motion`.
   - remote-core : `Pressed::set(id, true)` devient `Pressed::press(id)`, et `Pressed::set(id, false)` devient `Pressed::release(id)`.

   Nouveau : `Ms::as_f64()`, pour passer un temps de salon à une horloge de page ou à une statistique.
5. Le manifeste a un principe 8, « Aucune chaîne n'est un contrat », et `soiree check` attend son point dans « Écarts au manifeste » de `LEARNINGS.md`. Pour le tenir :
   - **La page.** `clippy.toml` interdit maintenant les méthodes à chaînes de `web_sys` (`get_element_by_id`, `set_attribute`, `class_list`, `set_inner_html`, `style().set_property`…). Pour chaque crate qui touche une page, génère son module de noms, puis déclare-le avec `pub mod page;` à la racine du crate :

     ```sh
     soiree dom crates/web/src/page.rs --html ../../../web/index.html --css ../../../web/style.css
     ```

     Les chemins sont relatifs au fichier généré. Ce que seul le Rust crée se déclare avec `--id <id> <balise>`, `--data <nom>`, `--attr <nom>`, `--class <nom>` ou `--tag <balise>`.
   - **Le code.** `document.get_element_by_id("title")` devient `page::id::TITLE.get()?`, qui renvoie directement le bon type. `el.class_list().add_1("tier-row")` devient `page::class::TIER_ROW.add_to(&el)?`. `el.set_attribute("data-row", &id)` devient `page::data::ROW.set(&el, &id)?`, et la lecture `page::data::ROW.get::<RowId>(&el)`. Une valeur de `data-*` qui aiguille (`data-action`) devient une enum avec `Display` et `FromStr`, et un test relit le HTML pour vérifier ses valeurs.
   - **Le suivi.** Après une modification du HTML ou du CSS, `soiree dom crates/web/src/page.rs`. `soiree check` échoue si un module n'est plus à jour.
   - **La version du jeu.** Copie `.soiree/salon/version.rs` (écrit par `soiree check`) comme `build.rs` du crate qui définit `Up`/`Down`, puis remplace la constante écrite à la main par `pub const VERSION: u32 = include!(concat!(env!("OUT_DIR"), "/version.rs"));`.
   - **Les routes.** `salon_core::route::{OFFER, TV_KEY, SOCKET}` remplacent les chemins `"/salon/…"` écrits en littéral.

6. Le modèle est maintenant jouable : `soiree new` donne une simulation sans écran, un hôte et les pages TV/téléphone. Pour un jeu existant, garde sa mécanique et prends les intégrations de `salon/README.md` ; ne remplace pas son code par le modèle.
7. Enregistre les entrées là où elles entrent dans la simulation avec `salon_host::journal::Recorder`, un en-tête typé, puis `record` et `checkpoint` de l’état complet (`salon_core::journal::fingerprint`). Une opération qui tire du hasard, même avant d’appliquer une action de bot, entre aussi dans le journal. Une fin naturelle appelle `finish` ; une nouvelle partie ouvre un nouveau journal avec son état initial exact. Un éditeur solo utilise `salon_web::journal::Recorder` et le récepteur typé `salon_host::journal::browser_sink`. Les journaux sont locaux, ignorés par Git et purgés après trente jours. Expose une commande de replay qui compare l’état, et conserve une fixture fictive versionnée.
8. `Connection::open` installe le filet anti-panique ; une page solo appelle `salon_web::install_tv_diagnostics`. Retire les hooks locaux qui le remplaceraient. `Game::diagnostic` reçoit les paniques et les marqueurs F8 : écris-les dans le journal de partie. La récupération s’arrête après trois recharges rapprochées, et si son compteur ne peut pas être mémorisé.
9. Pour tester l’hôte entier, utilise `salon_host::driver::Driver` et `salon_host::simulation::Network`, avec horloge et réseau à graine. La vraie salle utilise ce même driver. La campagne normale garde des milliers de graines ; `SOIREE_SIM_SEEDS` étend celle des jeux de l’hôte.
10. `soiree certify` ouvre quatre téléphones WebKit et quatre Chromium. `soiree doctor` vérifie le WebKit de sa version de Playwright ; installe-le avec la commande indiquée s’il manque. Ajoute les sondes causales `performance.probes` décrites dans `CERTIFICATION.md` : une entrée réelle, puis son effet sur la TV. Un canvas publie sa projection après un rendu réussi. Les chiffres sont écrits dans `certification/performance.json` et le rapport, et les dépassements font échouer la certification. La suspension DevTools reste vérifiée sur Chromium, les autres scénarios concernent les deux moteurs.
11. `soiree mutants` lance à la demande les mutations sur `crates/core`, avec `--in-diff` et des exclusions motivées. Rien ne l’appelle depuis `check` ou `certify`. Son rapport indique le périmètre et l’empreinte des sources : un petit premier passage ne prouve pas toute la simulation.

### Change de sens

- Les replays ne sont pas un bénéfice automatique du déterminisme : les entrées et tous les tirages aléatoires doivent réellement être enregistrés dans l’ordre de simulation. Le numéro calculé depuis les sources informe le diagnostic ; le format de replay contrôle sa lecture, et l’empreinte de l’état détecte une dérive de sens.
- Une certification mesure désormais des budgets et couvre WebKit. Un ancien passage Chromium seul ou sans sonde de rendu ne satisfait plus ce plancher.
- Une panique de page provoque une récupération visible et bornée. Elle ne reste plus silencieuse et ne déclenche jamais une boucle de rechargement illimitée.
- La liberté de casser les frontières internes s’accompagne d’un test et d’une notice de migration. Le contrat de l’app iPhone et les contraintes du monde réel restent à coordonner et à éprouver explicitement.
