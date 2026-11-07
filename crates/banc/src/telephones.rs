//! Les pages du téléphone dans de vrais navigateurs (PHASE-3, § 8.2, lot 12), pendant le
//! scénario `soiree` (`--pages chrome,firefox,safari`).
//!
//! Chaque page est un navigateur à part, lancé sur un profil neuf (son `localStorage`, donc
//! son jeton, est à lui) : Chrome **en émulation tactile** (`Emulation.setTouchEmulationEnabled`,
//! un écran de 390 × 844), Firefox (sa fenêtre à 360 × 640, des pointeurs `touch` par
//! `input.performActions`), Safari s'il est permis (`safaridriver` : « pas exécuté » tant que
//! l'hôte n'a pas coché l'automatisation à distance, réglage jamais touché). La page
//! s'ouvre à `http://127.0.0.1:<port>/s/<SALLE>#<fragment>` : une autre origine que l'écran
//! (`localhost`), que le relais admet en rôle `Appareil` depuis la boucle (§ 2.5).
//!
//! **Le pilote** joue comme un joueur, par la chaîne d'entrées du navigateur, jamais par
//! `dispatchEvent` (§ 8.2) : il lit ce que la page montre (`window.__banc.noeuds()`, en mode
//! banc seulement), tape un texte dans un champ vide (en le touchant d'abord), sinon touche
//! le bouton qu'il a le moins récemment touché, au plus un toutes les trois secondes. Il ne
//! connaît aucun jeu.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::Value as Json;

use crate::horloge::maintenant;
use crate::navigateur::{Lance, Navigateur};
use crate::soiree::Session;

/// Le pilote joue un coup par page toutes les 400 ms (choisi : plus lent qu'un joueur
/// pressé, assez vite pour une partie de Punchline en quelques minutes).
pub(crate) const PILOTAGE: Duration = Duration::from_millis(400);

/// Un bouton n'est pas retouché avant 1,5 s (choisi).
const RETOUCHE: Duration = Duration::from_millis(1_500);

/// Après un bouton, le pilote n'en touche aucun autre pendant 3 s (choisi, correction du
/// lot 12) : un joueur ne martèle pas les boutons. Au lot 12, il en touchait un toutes les
/// 400 ms ; dans le salon de Punchline, chaque personnage touché change la télé, et
/// l'organisateur simulé, qui attend 2 s d'une télé calme pour lancer, ne lançait jamais
/// (vu sur le contrôle négatif du scénario : la partie restée au salon 400 s).
const PAUSE_ENTRE_BOUTONS: Duration = Duration::from_secs(3);

/// Les textes que le pilote tape : courts, valides comme prénom comme comme réponse.
const TEXTES: &[&str] = &[
    "Léa",
    "Hugo",
    "Inès",
    "un chat",
    "la lune",
    "Paul",
    "deux fois",
    "Zoé",
];

/// Le pilote tient une croix 8 s, puis la lâche 4 s (choisi).
const TENUE: Duration = Duration::from_secs(8);
const LACHE: Duration = Duration::from_secs(4);

/// Les trois tailles du scénario `pages` (PHASE-3, § 7, lot 12 ; choisies) : un petit
/// téléphone, un téléphone courant, une tablette. Chaque page (Chrome et, depuis la
/// correction du lot 12, Firefox) passe de l'une à l'autre tous les [`COUPS_PAR_TAILLE`]
/// coups du pilote.
pub(crate) const TAILLES: [(u32, u32); 3] = [(360, 640), (390, 844), (768, 1024)];
const COUPS_PAR_TAILLE: u64 = 25;

/// L'écran du téléphone émulé (choisi : un téléphone courant).
pub(crate) const ECRAN_CHROME: (u32, u32) = (390, 844);
/// La fenêtre de Firefox (choisi : le plus petit des trois écrans du scénario `pages`).
pub(crate) const ECRAN_FIREFOX: (u32, u32) = (360, 640);

/// Une page du téléphone dans un navigateur.
pub(crate) struct PageTel {
    /// Le navigateur (« chrome », « firefox »).
    pub(crate) nom: String,
    /// Sa version.
    pub(crate) version: String,
    pub(crate) lance: Lance,
    /// Les onglets ouverts sur la page (le premier, puis un jumeau).
    pub(crate) onglets: Vec<usize>,
    /// L'onglet que le pilote joue.
    pub(crate) actif: usize,
    /// L'adresse de la page.
    pub(crate) url: String,
    appuyes: BTreeMap<String, Instant>,
    textes: usize,
    /// Le dernier état que la page a dit.
    pub(crate) etat: Option<Json>,
    /// M4 : la taille du module reçu et sa compilation sur la page, telles qu'elle les dit.
    pub(crate) m4: Option<Json>,
    /// Le plus de gestes que la page a dits émis.
    pub(crate) gestes_max: u64,
    /// Le pilote joue-t-il ?
    pub(crate) pilote: bool,
    /// Ce qui n'a pas marché en pilotant.
    pub(crate) erreurs: Vec<String>,
    /// Ce que le pilote a tapé : le relais ne doit le voir nulle part.
    pub(crate) textes_tapes: Vec<String>,
    /// Ce que la console de la page a dit (les erreurs comprises).
    pub(crate) console: Vec<String>,
    /// La croix tenue par le pilote : depuis quand, et où.
    pub(crate) croix: Option<(Instant, f64, f64)>,
    libre_jusqu_a: Option<Instant>,
    /// Les coups joués.
    pub(crate) coups: u64,
    /// La taille de l'écran, parmi [`TAILLES`].
    pub(crate) taille: (u32, u32),
    /// Les débordements vus (`scrollWidth` contre `clientWidth`), à leur taille d'écran.
    pub(crate) debordements: Vec<String>,
    /// Les pages vues, par leur suite de clés, à chaque taille : ce que le scénario
    /// `pages` a jugé.
    pub(crate) pages_jugees: std::collections::BTreeSet<String>,
    /// Les liens que le relais a donnés à la page (un par connexion) : M7 s'y lit.
    pub(crate) liens: std::collections::BTreeSet<u32>,
    /// Les croix tenues : de quand à quand (M7 ne lit la redite que pendant elles).
    pub(crate) tenues: Vec<(Instant, Instant)>,
    /// Pendant une croix tenue, à chaque coup : quand, et si le tenu que la page envoie
    /// est sa valeur relâchée (correction du lot 12 : un tenu relâché ne se redit pas, et
    /// M7 se lit aussi sur les seuls moments où le tenu ne l'est pas).
    pub(crate) tenus_vus: Vec<(Instant, bool)>,
    /// Quand le pilote a touché un bouton pour la dernière fois.
    dernier_bouton: Option<Instant>,
}

impl PageTel {
    /// Lance `n` sur un profil neuf sous `profils`, ouvre la page de la salle, et lui donne
    /// un écran de téléphone.
    pub(crate) fn ouvrir(
        n: Navigateur,
        port: u16,
        salle: &str,
        fragment: &str,
        profils: &Path,
    ) -> Result<PageTel, String> {
        let lance = Lance::lancer(n, None, profils).map_err(|e| format!("pas exécuté : {e}"))?;
        PageTel::sur(lance, n, port, salle, fragment)
    }

    /// Ouvre la page de la salle dans un navigateur déjà lancé (`Lance::lancer` : un
    /// profil neuf), et lui donne un écran de téléphone. Les retardataires lancent d'abord le
    /// navigateur, puis l'ouvrent : ce que coûte son lancement ne tombe pas dans la
    /// livraison du module qu'on mesure.
    pub(crate) fn sur(
        mut lance: Lance,
        n: Navigateur,
        port: u16,
        salle: &str,
        fragment: &str,
    ) -> Result<PageTel, String> {
        let url = format!("http://127.0.0.1:{port}/s/{salle}#{fragment}");
        if let Err(e) = lance.ouvrir(&url) {
            lance.fermer();
            return Err(e);
        }
        let (l, h) = match n {
            Navigateur::Chrome => ECRAN_CHROME,
            Navigateur::Firefox | Navigateur::Safari => ECRAN_FIREFOX,
        };
        if let Err(e) = lance.telephone(0, l, h) {
            lance.fermer();
            return Err(e);
        }
        Ok(PageTel {
            nom: n.nom().to_owned(),
            version: lance.version().to_owned(),
            lance,
            onglets: vec![0],
            actif: 0,
            url,
            appuyes: BTreeMap::new(),
            textes: 0,
            etat: None,
            m4: None,
            gestes_max: 0,
            pilote: true,
            erreurs: Vec::new(),
            textes_tapes: Vec::new(),
            console: Vec::new(),
            croix: None,
            libre_jusqu_a: None,
            coups: 0,
            taille: (l, h),
            debordements: Vec::new(),
            pages_jugees: std::collections::BTreeSet::new(),
            liens: std::collections::BTreeSet::new(),
            tenues: Vec::new(),
            tenus_vus: Vec::new(),
            dernier_bouton: None,
        })
    }

    /// Évalue `js` (qui rend une chaîne JSON) dans l'onglet `onglet`.
    pub(crate) fn evaluer_json(&mut self, onglet: usize, js: &str) -> Result<Json, String> {
        let v = self.lance.evaluer(onglet, js)?;
        if let Json::String(t) = &v {
            return serde_json::from_str(t).map_err(|e| format!("{t} : {e}"));
        }
        Ok(v)
    }

    /// L'état de la page dans l'onglet `onglet` (le `k`-ième ouvert).
    pub(crate) fn etat_de(&mut self, k: usize) -> Result<Json, String> {
        let o = self.onglets.get(k).copied().ok_or("pas d'onglet")?;
        self.evaluer_json(o, "window.__banc ? window.__banc.etat() : 'null'")
    }

    /// Touche le centre de l'élément de clé `cle` (ou « reprendre ») dans l'onglet `onglet`.
    pub(crate) fn toucher(&mut self, onglet: usize, cle: &str) -> Result<(), String> {
        let c = self.evaluer_json(
            onglet,
            &format!("window.__banc.centre({})", Json::from(cle)),
        )?;
        let (Some(x), Some(y)) = (
            c.get("x").and_then(Json::as_f64),
            c.get("y").and_then(Json::as_f64),
        ) else {
            return Err(format!("{cle} : pas de centre ({c})"));
        };
        self.lance.toucher(onglet, x, y)
    }

    /// Le pilote s'arrête, et lève le doigt qui tenait la croix (un rechargement, un onglet
    /// jumeau, l'oublieraient).
    pub(crate) fn arreter_le_pilote(&mut self) {
        self.pilote = false;
        if let Some((depuis, x, y)) = self.croix.take() {
            self.tenues.push((depuis, maintenant()));
            let o = self.onglets.get(self.actif).copied().unwrap_or(0);
            let _: Result<(), String> = self.lance.doigt(o, "touchEnd", x, y);
        }
    }

    /// La ferme.
    pub(crate) fn fermer(self) {
        self.lance.fermer();
    }

    fn noter_l_etat(&mut self, e: &Json) {
        if let Some(l) = e
            .get("lien")
            .and_then(Json::as_u64)
            .and_then(|l| u32::try_from(l).ok())
        {
            self.liens.insert(l);
        }
        if let Some(g) = e.get("gestes").and_then(Json::as_u64) {
            self.gestes_max = self.gestes_max.max(g);
        }
        if let Some(m) = e.get("m4").filter(|m| !m.is_null()) {
            self.m4 = Some(m.clone());
        }
        self.etat = Some(e.clone());
    }

    /// Un coup du pilote.
    fn un_coup(&mut self) -> Result<(), String> {
        let rang = self.actif;
        let onglet = self.onglets.get(rang).copied().ok_or("pas d'onglet")?;
        let etat = self.etat_de(rang)?;
        self.noter_l_etat(&etat);
        if !etat.get("ecran").is_some_and(Json::is_null) {
            return Ok(());
        }
        let noeuds = self.evaluer_json(onglet, "window.__banc.noeuds()")?;
        let noeuds = noeuds.as_array().cloned().unwrap_or_default();
        self.coups = self.coups.wrapping_add(1);
        self.juger_les_debordements(onglet, &noeuds)?;
        // Chaque page passe d'une taille à l'autre (correction du lot 12 : Firefox aussi,
        // par `browsingContext.setViewport` ; au lot 12, il restait à 360 × 640).
        if self.coups.is_multiple_of(COUPS_PAR_TAILLE) && self.croix.is_none() {
            let i = usize::try_from(self.coups / COUPS_PAR_TAILLE)
                .unwrap_or(0)
                .checked_rem(TAILLES.len())
                .unwrap_or(0);
            let (l, h) = TAILLES.get(i).copied().unwrap_or(ECRAN_CHROME);
            self.lance.telephone(onglet, l, h)?;
            self.taille = (l, h);
            return Ok(());
        }
        let maintenant = maintenant();
        // La croix : tenue 8 s par un doigt posé, puis lâchée 4 s ; rien d'autre ne se
        // touche pendant qu'elle est tenue (un seul doigt).
        if let Some((depuis, x, y)) = self.croix {
            if let Some(r) = etat.get("tenu_relache").and_then(Json::as_bool) {
                self.tenus_vus.push((maintenant, r));
            }
            if maintenant.saturating_duration_since(depuis) >= TENUE {
                self.croix = None;
                self.tenues.push((depuis, maintenant));
                self.libre_jusqu_a = maintenant.checked_add(LACHE);
                self.lance.doigt(onglet, "touchEnd", x, y)?;
            }
            return Ok(());
        }
        let croix = noeuds
            .iter()
            .find(|n| n.get("sorte").and_then(Json::as_str) == Some("croix"))
            .and_then(|n| n.get("cle").and_then(Json::as_str))
            .map(str::to_owned);
        if let Some(c) = croix
            && self.libre_jusqu_a.is_none_or(|t| maintenant >= t)
        {
            let centre = self.evaluer_json(
                onglet,
                &format!("window.__banc.centre({})", Json::from(c.as_str())),
            )?;
            if let (Some(x), Some(y), Some(l)) = (
                centre.get("x").and_then(Json::as_f64),
                centre.get("y").and_then(Json::as_f64),
                centre.get("l").and_then(Json::as_f64),
            ) {
                let x = x + l * 0.35;
                self.lance.doigt(onglet, "touchStart", x, y)?;
                self.croix = Some((maintenant, x, y));
                return Ok(());
            }
        }
        let champ_vide = noeuds.iter().find(|n| {
            n.get("sorte").and_then(Json::as_str) == Some("champ")
                && n.get("nom").and_then(Json::as_str) == Some("")
        });
        if let Some(c) = champ_vide.and_then(|c| c.get("cle")).and_then(Json::as_str) {
            let c = c.to_owned();
            self.toucher(onglet, &c)?;
            let t = TEXTES
                .get(self.textes.checked_rem(TEXTES.len()).unwrap_or(0))
                .copied()
                .unwrap_or("Léa");
            self.textes = self.textes.wrapping_add(1);
            self.lance.taper(onglet, t)?;
            if !self.textes_tapes.iter().any(|x| x == t) {
                self.textes_tapes.push(t.to_owned());
            }
            return Ok(());
        }
        if self
            .dernier_bouton
            .is_some_and(|t| maintenant.saturating_duration_since(t) < PAUSE_ENTRE_BOUTONS)
        {
            return Ok(());
        }
        let bouton = noeuds
            .iter()
            .filter(|n| n.get("sorte").and_then(Json::as_str) == Some("bouton"))
            .filter_map(|n| n.get("cle").and_then(Json::as_str))
            .filter(|c| {
                self.appuyes
                    .get(*c)
                    .is_none_or(|t| maintenant.saturating_duration_since(*t) >= RETOUCHE)
            })
            .min_by_key(|c| self.appuyes.get(*c).copied())
            .map(str::to_owned);
        if let Some(c) = bouton {
            self.toucher(onglet, &c)?;
            self.appuyes.insert(c, maintenant);
            self.dernier_bouton = Some(maintenant);
        }
        Ok(())
    }
}

impl PageTel {
    /// Le scénario `pages` (§ 8.2) : sur chaque page vue, à la taille d'écran du moment, un
    /// élément qui déborde de sa boîte (`scrollWidth` contre `clientWidth`), ou de la page,
    /// se nomme par sa clé.
    fn juger_les_debordements(&mut self, o: usize, noeuds: &[Json]) -> Result<(), String> {
        let signature: Vec<&str> = noeuds
            .iter()
            .filter_map(|n| n.get("cle").and_then(Json::as_str))
            .collect();
        let (l, h) = self.taille;
        let cle = format!("{l}x{h} {}", signature.join(","));
        if !self.pages_jugees.insert(cle) {
            return Ok(());
        }
        let d = self.evaluer_json(o, "window.__banc.debordements()")?;
        for e in d
            .get("elements")
            .and_then(Json::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let texte = format!(
                "{} à {l} × {h} : {} {} (scroll {}, client {}, droite {})",
                self.nom,
                e.get("sorte").and_then(Json::as_str).unwrap_or("?"),
                e.get("cle").and_then(Json::as_str).unwrap_or("?"),
                e.get("scroll").unwrap_or(&Json::Null),
                e.get("client").unwrap_or(&Json::Null),
                e.get("droite").unwrap_or(&Json::Null),
            );
            if !self.debordements.contains(&texte) && self.debordements.len() < 64 {
                self.debordements.push(texte);
            }
        }
        Ok(())
    }
}

/// M7 (§ 8.6) : la redite du tenu, telle que le relais l'a reçue. Pour chaque lien, les
/// instants des trames remplaçables (`Classe::Remplacable`, le canal des tenus) reçues de
/// lui, en microsecondes depuis le lancement du relais.
pub(crate) fn trames_de_tenu(enregistrement: &Path) -> Result<BTreeMap<u32, Vec<u64>>, String> {
    let octets =
        std::fs::read(enregistrement).map_err(|e| format!("{} : {e}", enregistrement.display()))?;
    let (notes, _) = relais::enregistrement::lire(&octets);
    let mut instants: BTreeMap<u32, Vec<u64>> = BTreeMap::new();
    for n in notes {
        if n.sens != relais::enregistrement::Sens::Recu {
            continue;
        }
        let Ok(liaison::VersRelais::Trame {
            classe: liaison::Classe::Remplacable(_),
            ..
        }) = liaison::VersRelais::decoder(&n.octets)
        else {
            continue;
        };
        instants.entry(n.lien.0).or_default().push(n.t_us);
    }
    Ok(instants)
}

/// Un coup du pilote sur chaque page qui joue, et sa console lue.
pub(crate) fn piloter(s: &mut Session) {
    for p in &mut s.pages {
        if let Ok(lignes) = p.lance.ecouter(Duration::from_millis(1)) {
            p.console.extend(lignes);
        }
        if p.pilote
            && let Err(e) = p.un_coup()
            && p.erreurs.len() < 32
        {
            p.erreurs.push(e);
        }
    }
}

/// Les secrets que le relais ne doit voir nulle part dans ce qu'il a enregistré : le
/// secret d'entrée, G, les jetons, les textes tapés (§ 8.1, `le_relais_ne_voit_aucun_secret`).
pub(crate) fn secrets_vus(
    enregistrement: &Path,
    aiguilles: &[(String, Vec<u8>)],
) -> Result<Vec<String>, String> {
    let octets =
        std::fs::read(enregistrement).map_err(|e| format!("{} : {e}", enregistrement.display()))?;
    Ok(aiguilles
        .iter()
        .filter(|(_, a)| a.len() >= 3 && octets.windows(a.len()).any(|w| w == a.as_slice()))
        .map(|(nom, _)| nom.clone())
        .collect())
}

/// Le témoin positif de la recherche des secrets (*correction du lot 12*) : 24 octets pris
/// au milieu de la dernière trame qu'un lien d'une page a envoyée au relais, tels qu'ils
/// sont dans l'enregistrement. [`secrets_vus`] doit les y voir : sinon la recherche ne lit
/// pas ce que le relais a reçu des pages, et son « aucun secret » ne prouve rien.
pub(crate) fn temoin_des_pages(
    enregistrement: &Path,
    liens: &std::collections::BTreeSet<u32>,
) -> Result<Option<Vec<u8>>, String> {
    let octets =
        std::fs::read(enregistrement).map_err(|e| format!("{} : {e}", enregistrement.display()))?;
    let (notes, _) = relais::enregistrement::lire(&octets);
    Ok(notes
        .iter()
        .rev()
        .filter(|n| n.sens == relais::enregistrement::Sens::Recu && liens.contains(&n.lien.0))
        .find_map(|n| {
            let milieu = n.octets.len() / 2;
            n.octets
                .get(milieu.saturating_sub(12)..milieu.saturating_add(12))
                .map(<[u8]>::to_vec)
        }))
}

/// Un jeton écrit en hexadécimal (`localStorage`), en octets.
pub(crate) fn octets_de_l_hexa(h: &str) -> Option<Vec<u8>> {
    if !h.len().is_multiple_of(2) {
        return None;
    }
    (0..h.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(h.get(i..i.saturating_add(2))?, 16).ok())
        .collect()
}
