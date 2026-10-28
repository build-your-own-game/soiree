# Changements

Chaque version a sa section, de la plus récente à la plus ancienne. « Passer à cette version » s'adresse à l'agent d'un jeu (voir `AGENTS.md`).

## v0.3.1 — Non publié

Le projet Xcode déclare les fichiers produits par sa phase de compilation Rust :
les liaisons Swift, leur en-tête, leur module et l’archive native. Xcode attend
ainsi leur génération avant de compiler Swift, y compris sur un checkout neuf.
La CI vérifie explicitement ce cas sans liaisons prégénérées.

### Passer à cette version

Aucun changement de code de jeu. Le correctif concerne le projet de l’app
iPhone ; aucun `cargo xtask ios` préalable n’est nécessaire à son build Xcode.

## v0.3.0 — 2026-10-28

Les engagements propres au jeu passent de tableaux Markdown à
`verification.toml`. L’outil résout leurs obligations et les contrôles communs,
exécute les contrôleurs, puis évalue des observations rattachées à leurs entrées.
`CERTIFICATION.md` devient une vue des engagements ; les rapports datés restent
séparés. Cette rupture demande une migration explicite des identités de tests :
un nom de fonction trouvé dans le texte ou un ancien rapport ne suffit plus.

Cargo fournit les cibles et libtest les tests réellement découverts et exécutés.
Paquet, genre de cible, cible, nom complet, features, features par défaut et
profil distinguent les identités. Tests ignorés, zéro sélection et compilation
échouée ne satisfont aucun engagement. Le noyau de décision de production est
vérifié avec une version figée de Verus ; les adaptateurs, oracles et décisions
d’applicabilité restent hors de cette preuve.

`check --fast` et `certify -- --only …` rendent leur périmètre explicite.
`certify` échoue si un contrôle demandé est absent, sauté, partiel ou si le
contrôleur tombe en panne ; un simple code de sortie Node réussi ne suffit plus.
Un passage ciblé réussi laisse les autres obligations non attestées. Les items
du catalogue sans décision restent non revus et les gestes sur appareils réels
restent des jugements humains. Les coûts d’itération et de CI restent à mesurer.

La CI conserve les téléchargements Cargo, les compilations et les outils figés,
avec contrôle exact de leurs versions. Le binaire du commun est toujours
reconstruit depuis son checkout. Aucun rapport n’est restauré comme résultat
valide ; le gain de temps de ces caches reste à mesurer.

Les attentes UI attendent désormais la valeur résolue de leur expression : une
promesse qui résout `false` ne franchit pas l’attente. Les expressions régulières
et les délais sont validés avant l’exécution des contrôles UI. Le transport de
test est inscrit dans le périmètre ; un passage `phone_upstream: "loopback"`
laisse les garanties d’accès au réseau local non attestées. Les fichiers servis
sont préparés avant leur empreinte.

`soiree upgrade` met aussi à niveau les dépendances communes déclarées
directement dans les manifestes des membres du workspace. En 0.2.0, seules
celles du manifeste racine étaient alignées ; Sidequest conservait ainsi
`remote-web` et `remote-protocol` à l’ancienne étiquette dans ses membres.
Les commentaires et les dépendances de développement, de construction et
propres à une cible restent préservés. La découverte Cargo fonctionne
avant le premier lockfile et ne le crée ni ne le modifie ; un membre hors
du dossier du jeu ou une étiquette absente arrête la commande avant toute
écriture.

Le lock du projet fuzz de la télécommande et sa cible `phone_session` sont remis en état :
le booléen du corpus est converti vers `Orientation` au bord de l’entrée,
sans changer les octets des corpus.

### Passer à cette version

1. Installer l’outil avec
   `cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.3.0 soiree`.
2. À la racine du jeu, lancer `soiree upgrade --dry-run`, puis
   `soiree upgrade`. La commande aligne le manifeste racine, les membres
   du workspace, les règles et la CI sur cette étiquette publiée. Un
   sous-projet indépendant, tel qu’un workspace `fuzz/`, garde son propre
   lockfile : le résoudre aussi après la migration s’il dépend du jeu.
3. Préparer `references.json` : pour chaque référence historique active, donner
   son paquet Cargo, son genre de cible (`lib`, `bin` ou `test`), sa cible,
   son nom complet libtest et sa configuration. Une redirection de helper ou
   de nom doit être expliquée. Le format exact est dans `VERIFICATION.md`.
4. Lancer `soiree contracts --migrate references.json`, relire les engagements,
   hypothèses et décisions importés, puis `soiree contracts --write-view`.
   L’original est conservé dans `CERTIFICATION.legacy.md` ; aucun résultat
   historique n’est importé comme réussi. Un jeu qui possède déjà un contrat
   structuré l’édite directement, sans relancer cette migration.
5. Lancer `soiree check`, les scénarios nécessaires avec `soiree certify`, puis
   le passage `soiree check --locked --published` depuis un clone sans patch
   local. Examiner les contrôles en attente et les limites du rapport avant
   de déclarer le périmètre validé. Les crates communes du SDK gardent leur
   version de paquet 0.2.0 ; l’étiquette Git v0.3.0 fige l’ensemble distribué.

## v0.2.0 — 2026-10-28

Le contrat entre une page libre et l’app iPhone est défini dans
`remote-protocol::web`, réutilisé par le SDK WASM et par les liaisons Swift
UniFFI. Le format JSON public reste identique. Les propriétés CSS écrites par
Rust demandent maintenant une catégorie de valeur typée.

Les pages possèdent leurs ressources navigateur via `salon_web::ui` :
listeners, timers, animations, tickets d’opérations asynchrones, composition
et effets ponctuels. Les formulaires gardent leur identité pendant les
mises à jour qui ne changent pas leur tâche. Les tests exercent les
destructions, les résultats tardifs, les compositions et les décodeurs
ambigus, ainsi que la propagation des échecs des contrôles eux-mêmes.
`soiree new` et `soiree upgrade` installent une CI versionnée ; le mode
`check --published` refuse les dépendances communes non publiées ou locales.
Ce mode exige un jeu versionné (`Cargo.toml` et `soiree.toml`) ; retirer la
version ne permet pas de sauter les règles de CI et les déclarations U1.
Les noms de jeu `salon` et `remote` sont réservés aux crates du commun ;
`--org` choisit le dépôt du jeu sans rediriger ses dépendances communes.

### Passer à cette version

1. Après publication de l’étiquette `v0.2.0`, installer l’outil de cette version
   et aligner les dépendances salon/remote ainsi que `soiree.toml`. Pendant le
   développement chez l’hôte, les jeux gardent leur ancienne étiquette Cargo
   et utilisent le `[patch]` local ; ils ne doivent pas être poussés ainsi.
2. Régénérer chaque module avec `soiree dom <page.rs>`. Pour une propriété
   personnalisée écrite par Rust, déclarer sa catégorie dans sa feuille CSS,
   par exemple `/* soiree-type: --accent color */`. Catégories : `color`,
   `length`, `number`, `time`, `image`. Une propriété sans catégorie n’a pas
   de setter ; les déclarations absentes ou contradictoires échouent.
3. Remplacer les valeurs libres de `prop::X.set` par une référence à
   `salon_web::style::{Color, Length, Number, Time, Image}` :
   `Color::rgb(255, 0, 0)`, `Length::px(x)?`, `Number::new(n)?`,
   `Time::milliseconds(ms)` ou `Image::url(src)?`. Les couleurs hexadécimales
   existantes se valident avec `parse::<Color>()`. Les coordonnées et nombres
   non finis sont refusés. Les noms de propriété viennent du générateur.
4. `remote_web::Info::player` est un `PlayerId` validé : utiliser `.bytes()`
   pour le jeton ou `.to_string()` pour l’affichage. Retirer les décodeurs
   hexadécimaux qui validaient à nouveau cette même identité.
5. Reconstruire l’app avec `cargo xtask ios`. `WebRemote.swift` utilise les
   commandes générées de `remote-ffi`, et les scripts de sortie sont fournis
   par le même contrat Rust. `cargo xtask web-contract` compile et exécute les
   liaisons Swift réelles et vérifie les scripts dans JavaScriptCore ; ce
   contrôle rejoint `soiree check remote` sur macOS. Ailleurs, il est signalé
   comme non exécuté. `cargo xtask web-contract-browser` complète ce contrôle
   avec le vrai SDK WASM, WebKit et Chromium (navigateurs du harnais requis).
   Un build Xcode vérifie le consommateur réel.
6. `soiree check`, puis les vérifications navigateur applicables. Les fichiers
   `.soiree/` et les sorties UniFFI restent générés et ignorés par Git.
7. Conserver chaque `Connection` aussi longtemps que sa page en a besoin :
   sa dernière copie détruite ferme maintenant le transport. Remplacer les
   callbacks oubliés et les timers/boucles manuels par `Listener`,
   `Listeners`, `Timer` et `AnimationLoop` de `salon_web::ui`, gardés dans
   leur propriétaire. Leurs résultats sont `must_use`. Les listeners DOM et
   les callbacks `on_orientation`, `motion`, `Datagram::on_message` de
   `remote-web` prennent `Fn` : remplacer une capture directement mutable
   par un `Cell` ou `RefCell` détenu par le propriétaire. Libérer les emprunts
   avant une opération DOM pouvant déclencher un événement synchrone.
8. Protéger les tâches avec `OperationSlot` (le dernier appel remplace le
   précédent) ou `OperationScope` (plusieurs tâches indépendantes). Appeler
   `finish` juste avant d’appliquer le résultat ; annuler à la fermeture ou
   au remplacement de l’objet logique. Un ticket n’interrompt pas à lui seul
   le travail natif du navigateur. Brancher `Composition` sur les événements
   de composition et bloquer les nouvelles validations/raccourcis ; garder
   les relâchements de commandes tenues. Un effet ponctuel ordonné peut
   utiliser `OnceEffects`, avec identités croissantes et remise à zéro lors
   d’un changement de session faisant autorité.
9. Ajouter `ui` à `cert.json` : scénarios ou commandes réelles couvrant
   `editing`, `lifecycle`, `async`, `effects`, et une justification pour
   chaque catégorie non applicable. Voir `CERTIFICATION.md`. Le modèle et
   les trois jeux de l’hôte en donnent des exemples. Installer la CI avec
   `soiree upgrade` ; `--dry-run` montre les fichiers visés. La commande
   cible la version du binaire installé et refuse une étiquette non publiée
   avant de modifier le jeu ; elle ne réécrit pas automatiquement sa logique.
10. Remplacer `postcard::from_bytes` par `salon_core::codec::decode` pour
    un message dont le cadre contient exactement un objet. Les décodeurs de
    salon et de la télécommande l’utilisent déjà. Les variantes JSON non
    étiquetées ne sont acceptables que si leurs formes sont disjointes,
    vérifiées par des cas négatifs.

### Change de sens

- Une identité doit être exactement seize octets, exprimés par 32 chiffres
  hexadécimaux. Un datagramme encodé en base64 doit être canonique et complet.
- Les objets JavaScript mal typés sont refusés avant les effets natifs ; les
  ports et octets ne sont plus tronqués par les conversions NSNumber.
- Les valeurs CSS d’un jeu passent par des constructeurs validés et des
  propriétés de catégorie correspondante. Les tracés SVG et la syntaxe
  déclarative CSS restent du vocabulaire du navigateur, pas une promesse de
  typage de toute sa grammaire.
- Les octets ajoutés après un message binaire valide sont refusés ; ils
  étaient auparavant ignorés par `postcard::from_bytes`.
- La destruction du dernier `Connection` clôt son transport. Les ressources
  d’une vue cessent leurs rappels avec leur propriétaire.
- Plusieurs abonnements aux capteurs de `remote-web` partagent la source :
  seul le dernier abonnement détruit envoie `motion:false`.

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
