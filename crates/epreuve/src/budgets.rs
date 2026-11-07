//! E19 : ce que coûtent les appels du cœur, jugé contre des budgets dérivés du temps
//! d'une image (§ 7.2 ; CONTRAT, « Les budgets »).
//!
//! La sonde note le carburant de chaque appel que le cœur fait sur la définitive des
//! soirées principales, et celui de chaque entrée avec son pas. Le carburant compte
//! les instructions du module réécrit (§ 3.6) : il est le même à chaque exécution,
//! sur toute machine. Le CONTRAT, lui, parle en temps. Ce fichier fait le lien, à un
//! seul endroit : [`BUDGETS_E19`].
//!
//! # Ce que juge E19
//!
//! Le **coût** d'un appel est son carburant, plus le coût fixe d'un appel du module
//! ([`BudgetsE19::cout_fixe`]) : le passage de l'appel au module et la remise à zéro
//! des zones que le moteur fait avant chaque appel (la pile, la boîte, le brouillon,
//! la zone de panique ; § 3.2), que le carburant ne compte pas. Cent entrées de peu
//! de carburant dans un même pas coûtent donc cent fois ce fixe.
//!
//! Un **tour** est un réveil de la boucle de l'autorité : un appel à
//! `Partie::avancer_borne`, qui joue un pas, plusieurs après un gel (le rattrapage), ou
//! une part d'un pas trop chargé d'arriérés (voir « Les arriérés »). L'épreuve en fait
//! un par pas simulé, comme le Worker un par image. Le coût d'un tour est la somme du
//! coût des entrées qu'il a jouées.
//!
//! Une **image** où un geste arrive en retard coûte le tour de la définitive, plus le
//! rejeu de la fenêtre sur la tête et la lecture de la scène (§ 10, « Ce qu'un FPS
//! exige »). L'image d'un tour `t` qui finit au pas `s` est le coût des tours `t` et
//! `t + 1`, plus, si `t` a fini son avancée et que le jeu a une fenêtre, le rejeu de la
//! tête, compté comme en phase 2 par les entrées que la définitive jouera aux pas
//! `s + 2` à `s + fenêtre + 1`. Sans découpage, c'est la somme de `(fenêtre + 2)` pas
//! consécutifs, comme en phase 2. Un tour qui laisse un pas inachevé ne fait pas avancer
//! la tête (elle ne se rejoue qu'à une avancée finie). Chaque image des soirées, la plus lourde comprise, et chaque
//! lecture, se jugent à leur maximum : un décompte final dix fois trop lourd, une
//! fois par partie, échoue (CONTRAT : « aucune image au-delà de 33 ms en jeu » ; le
//! 99ᵉ centile de INTERFACE § 7.2 ne vaut que pour les 60 images par seconde).
//!
//! | Ce qui est jugé | Sur quoi | Budget |
//! |---|---|---|
//! | chaque image des soirées | son maximum | [`BudgetsE19::image_max`] |
//! | les images des soirées | leur 99ᵉ centile | [`BudgetsE19::image_courante`] |
//! | chaque lecture courante du cœur (`Regarder`, `Etat`, `Valider`, `Ressources`) | son maximum | [`BudgetsE19::lecture`] |
//! | la vue de l'écran de chaque pire cas (E16) | son maximum | [`BudgetsE19::pire_cas`] |
//! | chaque page du téléphone (`Page`, appelée par E21) | son maximum | [`BudgetsE19::page`] |
//!
//! Une vue de pire cas est construite depuis les limites déclarées pour être la plus
//! lourde : elle a droit à l'image la plus longue que le CONTRAT permet. Une lecture
//! courante tombe dans une image ordinaire, qui doit tenir 60 images par seconde
//! (RESTE-PHASE-1, points 18 et 19 ; MESURES-PHASE-1, proposition 3).
//!
//! # Les arriérés
//!
//! Un arriéré est ce que le commun livre d'un coup au retour d'un téléphone coupé, en
//! arrière-plan ou endormi, ou de l'ordinateur de la soirée gelé : après un gel de 40 s
//! du plan standard, sept téléphones du socle des fautifs livrent 32 gestes chacun au
//! même pas, 225 entrées. Le cœur ne les étale pas sur plusieurs pas, ce qui changerait
//! leur sens (une réponse faite à temps passerait après l'échéance) : il finit un pas
//! trop chargé sur plusieurs tours, au plus `partie::ARRIERES_PAR_TOUR` arriérés par
//! tour, ses arrivées figées au tour où il commence (PHASE-3, § 2.11, P7). Un arriéré
//! y est une arrivée faite sur une vue de plus de `partie::ARRIERE_PAS` pas avant sa
//! réception ; une entrée fraîche n'est jamais retenue. E19 juge donc chaque tour, coût
//! fixe compris : la règle de la phase 2, qui ne comptait pas le coût fixe des
//! arriérés, est tombée avec le découpage.
//!
//! # D'où viennent les budgets
//!
//! **Ils sont provisoires.** Le CONTRAT ne fixe que des durées ; ce qui suit les
//! traduit en carburant, sur des mesures d'une seule machine et des marges choisies.
//! Le mainteneur les fixera (REPONSES-PROVISOIRES, question 12) ; la phase 3, dans
//! un navigateur sur l'ordinateur de la soirée, remplacera les marges par des
//! mesures. Le rapport les dit provisoires.
//!
//! 1. **Les mesures.** Le 2026-11-04, de 19 h 33 à 19 h 37 (UTC), sur la machine de
//!    l'hôte : un portable Apple M5, 10 cœurs (4 de performance, 6 d'efficacité), 16 Go,
//!    macOS 26.6.2 ; rustc 1.99.0 ; wasmtime 50.0.1 (Cranelift), `--release`.
//!    Programme : `examples/etalonner.rs`, sur `devine`, `boucle-au-vote`, et les
//!    modules de Punchline, Sidequest et Tier list (copies hors de leurs dépôts, avec
//!    leur seule déclaration `INVARIANTS` ajoutée). D'autres programmes tournaient sur
//!    la machine.
//!    - Une unité de carburant, sur les appels d'au moins un million d'unités :
//!      médiane de 25 à 63 ps selon le module, 90ᵉ centile de 31 à 87 ps, maximum
//!      156 ps ; sur ceux d'au moins cent mille unités, où le bruit du coût fixe
//!      pèse, 90ᵉ centile de 45 à 190 ps à la seconde passe (19 h 37), et jusqu'à
//!      304 ps (soirée de Tier list ; maximum 8 313 ps) à la première, une minute
//!      plus tôt (19 h 36). Une unité vaut moins qu'un cycle : la plupart des
//!      instructions de la pile de WebAssembly (`local.get`, `i32.const`…) ne
//!      coûtent rien une fois compilées.
//!      → [`PS_PAR_UNITE`] : 200 ps, **choisis**, pas mesurés : une borne
//!      au-dessus des 90ᵉ centiles de la seconde passe, et sous celui de 304 ps de
//!      la première. Sur les appels lourds, ceux que E19 fait échouer, la mesure
//!      est trois à huit fois plus basse : le Regarder du pire cas d'origine de
//!      Tier list (41,6 millions d'unités) a duré au plus 1,565 ms, 37 ps par
//!      unité ; à la marge de 4, 6,3 ms, sous la part de 8,25 ms qui lui revient.
//!      C'est cette borne choisie, pas la mesure, qui l'a fait échouer, et qui a
//!      fait baisser `TUILES_MAX` de Tier list de 1 000 à 300 (revue de la phase 2,
//!      constat 10). La remplacer par une mesure des appels lourds est une décision
//!      du mainteneur, avec les budgets.
//!    - La remise à zéro des zones, mesurée sur un tampon seul, déjà écrit, par
//!      `fill(0)`, comme `preparer` la fait : de 5,9 à 9,1 ns par Kio pour un tampon
//!      de la taille des zones de chaque module (de 2,1 à 25 Mio), mesuré juste après
//!      son chargement ; de 5,2 à 18,3 ns par Kio pour des tampons de 64 Kio à
//!      32 Mio mesurés au lancement du programme, les plus lents au premier passage.
//!      Dans le module, la médiane des `Bonjour` de Tier list (171 µs pour 25 Mio de
//!      zones, 6,7 ns par Kio en comptant tout le fixe) borne ce que la remise à zéro
//!      y coûte.
//!      → [`PS_PAR_KIO_REMIS_A_ZERO`] : 10 ns par Kio, **choisis** au-dessus de
//!      chaque mesure faite à la taille des zones, pas de chaque tampon mesuré à
//!      froid.
//!    - Le reste du fixe (le fil de l'instance, l'entrée dans wasmtime, la copie de
//!      l'appel et du retour) : la médiane de deux mille `Bonjour` moins la remise à
//!      zéro, de 2,3 à 5,9 µs ; le 99ᵉ centile ajoute jusqu'à 6 µs.
//!      → [`FIXE_NS`] : 10 µs, **choisis** au-dessus de ces médianes.
//!
//!    Les trois nombres de E19 sont donc des bornes choisies au-dessus des mesures,
//!    pas des mesures : le rapport le dit.
//! 2. **La marge** entre cette mesure et la partie qui fait foi : [`MARGE_NAVIGATEUR`]
//!    fois [`MARGE_MACHINE`], 4. Le facteur entre wasmtime et le navigateur n'est
//!    pas mesurable avant la phase 3 ; les comparaisons publiées de WebAssembly dans
//!    un navigateur et du code natif donnent un ralentissement moyen de l'ordre de
//!    1,5 et des pointes de 2,5 (Jangda et al., USENIX ATC 2019 : un ordre de
//!    grandeur lu, non mesuré ici), et Cranelift n'est pas du code natif non plus :
//!    2. L'ordinateur de la soirée n'est pas décrit (CONTRAT, « Le matériel : à
//!    renseigner ») ; un cœur de M5 est un cœur récent et rapide : 2.
//! 3. **La part du jeu** dans une image : [`PART_DU_JEU_POUR_CENT`], la moitié. Le
//!    reste va au commun : réseau, décodage, rendu de la scène, composition. La
//!    phase 1 n'a pas de rendu : rien n'en est mesuré.
//! 4. **Les images** du CONTRAT : [`IMAGE_MAX_NS`] (« aucune image au-delà de
//!    33 ms en jeu ») et [`IMAGE_COURANTE_NS`] (« au moins 60 images par seconde »).
//!    Une image se partage entre les entrées de ses pas et une lecture de la scène :
//!    [`PART_DES_ENTREES_POUR_CENT`], la moitié chacune, pour que le pire des deux
//!    ensemble tienne encore dans l'image.
//!
//! D'où, en unités de carburant : `durée × part du jeu × part des entrées / (ps par
//! unité × marge)`. Pour 33 ms : 33 ms × ½ × ½ / (0,2 ns × 4) = 10 312 500 unités ;
//! pour 1/60 s : 5 208 333. Le coût fixe d'un appel, lui, ne prend pas la marge,
//! déjà au dénominateur du budget : `(fixe + Kio remis à zéro × 10 ns) / 0,2 ns`,
//! soit 155 800 unités pour `devine` (2 116 Kio de zones) et 1 333 400 pour 25 668
//! Kio.
//!
//! **La page du téléphone** (PHASE-3, § 2.10, lot 6) tourne sur le fil principal du
//! téléphone, à chaque vue, chaque frappe et chaque changement de taille, sans Worker.
//! Aucun budget ni aucune durée ne visent encore un téléphone : `Page` a le budget d'une
//! lecture courante divisé par [`MARGE_TELEPHONE`], 4, choisi (question 22 au
//! mainteneur). Elle est jugée sur les pages que E21 appelle, coût fixe compris.
//!
//! # Ce qui n'est pas jugé
//!
//! La forme canonique, la garde, `Creer`, `Bonjour` et `Decrire` ne tombent pas
//! dans une image de jeu ; leur carburant est seulement décrit. L'entrée et la
//! lecture d'une même image ne sont pas sommées : chacune a sa moitié. 120 images par
//! seconde (« si l'écran le permet ») ne sont pas jugées. Le plafond de carburant de
//! chaque appel ([`crate::BUDGETS`], 2²⁸) reste celui qui arrête une boucle infinie :
//! ce n'est pas un budget de jeu.

use std::collections::BTreeMap;

use crate::sonde::Compte;

/// Choisi au-dessus des mesures : la durée d'une unité de carburant dans wasmtime, en
/// picosecondes. Mesuré le 2026-11-04 sur la machine de l'hôte : de 25 à 63 ps en
/// médiane sur les appels d'au moins un million d'unités, 156 ps au plus ; 90ᵉ centile
/// de 45 à 304 ps sur ceux d'au moins cent mille (voir « D'où viennent les
/// budgets »). Provisoire.
pub const PS_PAR_UNITE: u64 = 200;
/// Choisi au-dessus des mesures : le coût fixe d'un appel hors de la remise à zéro des
/// zones, en nanosecondes (médianes mesurées de 2,3 à 5,9 µs, même machine).
/// Provisoire.
pub const FIXE_NS: u64 = 10_000;
/// Choisi au-dessus des mesures : la remise à zéro d'un Kio de zones, en
/// picosecondes (de 5,9 à 9,1 ns mesurés à la taille des zones, même machine).
/// Provisoire.
pub const PS_PAR_KIO_REMIS_A_ZERO: u64 = 10_000;
/// Choisi : le facteur entre wasmtime et le moteur d'un navigateur. Provisoire, à
/// remplacer par une mesure en phase 3.
pub const MARGE_NAVIGATEUR: u64 = 2;
/// Choisi : le facteur entre la machine de la mesure et l'ordinateur de la soirée.
/// Provisoire, à remplacer par une mesure sur le matériel du CONTRAT.
pub const MARGE_MACHINE: u64 = 2;
/// Choisi : la part d'une image laissée au jeu, en pour cent. Provisoire.
pub const PART_DU_JEU_POUR_CENT: u64 = 50;
/// Choisi : la part de celle du jeu laissée aux entrées de l'image, l'autre allant à
/// une lecture de la scène, en pour cent. Provisoire.
pub const PART_DES_ENTREES_POUR_CENT: u64 = 50;
/// Choisi : le facteur entre la machine de la mesure et un téléphone, pour la page
/// (`Appel::Page`), qui tourne sur le fil principal du téléphone. Aucun téléphone n'est
/// mesuré (PHASE-3, § 2.10 ; question 22) : provisoire, jusqu'à la mesure sur de vrais
/// téléphones (§ 8.5).
pub const MARGE_TELEPHONE: u64 = 4;
/// Le CONTRAT : « aucune image au-delà de 33 ms en jeu ».
pub const IMAGE_MAX_NS: u64 = 33_000_000;
/// Le CONTRAT : « au moins 60 images par seconde ».
pub const IMAGE_COURANTE_NS: u64 = 16_666_667;

/// Les budgets que juge E19, en unités de carburant, et ce dont ils viennent. Tous
/// provisoires : voir la documentation du module (`budgets.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BudgetsE19 {
    /// [`PS_PAR_UNITE`].
    pub ps_par_unite: u64,
    /// [`FIXE_NS`].
    pub fixe_ns: u64,
    /// [`PS_PAR_KIO_REMIS_A_ZERO`].
    pub ps_par_kio_remis_a_zero: u64,
    /// [`MARGE_NAVIGATEUR`] fois [`MARGE_MACHINE`].
    pub marge: u64,
    /// [`PART_DU_JEU_POUR_CENT`].
    pub part_du_jeu_pour_cent: u64,
    /// [`PART_DES_ENTREES_POUR_CENT`].
    pub part_des_entrees_pour_cent: u64,
    /// Les entrées d'une image, `(fenêtre + 2)` pas, chacune : depuis 33 ms.
    pub image_max: u64,
    /// Les entrées d'une image, `(fenêtre + 2)` pas, au 99ᵉ centile : depuis 1/60 s.
    pub image_courante: u64,
    /// Chaque lecture courante du cœur : depuis 1/60 s.
    pub lecture: u64,
    /// La vue de l'écran de chaque pire cas : depuis 33 ms.
    pub pire_cas: u64,
    /// Chaque page du téléphone (`Appel::Page`, E21) : une lecture courante divisée par
    /// [`MARGE_TELEPHONE`].
    pub page: u64,
}

/// `duree_ns`, la part du jeu et une part de l'image, en unités de carburant.
const fn en_unites(duree_ns: u64, part_pour_cent: u64) -> u64 {
    // ns × 1000 = ps ; deux pourcentages, d'où 10 000.
    duree_ns
        .saturating_mul(1000)
        .saturating_mul(PART_DU_JEU_POUR_CENT)
        .saturating_mul(part_pour_cent)
        / (PS_PAR_UNITE * MARGE_NAVIGATEUR * MARGE_MACHINE * 10_000)
}

/// Les budgets de E19, provisoires : le seul endroit où ils s'écrivent.
pub const BUDGETS_E19: BudgetsE19 = BudgetsE19 {
    ps_par_unite: PS_PAR_UNITE,
    fixe_ns: FIXE_NS,
    ps_par_kio_remis_a_zero: PS_PAR_KIO_REMIS_A_ZERO,
    marge: MARGE_NAVIGATEUR * MARGE_MACHINE,
    part_du_jeu_pour_cent: PART_DU_JEU_POUR_CENT,
    part_des_entrees_pour_cent: PART_DES_ENTREES_POUR_CENT,
    image_max: en_unites(IMAGE_MAX_NS, PART_DES_ENTREES_POUR_CENT),
    image_courante: en_unites(IMAGE_COURANTE_NS, PART_DES_ENTREES_POUR_CENT),
    lecture: en_unites(IMAGE_COURANTE_NS, 100 - PART_DES_ENTREES_POUR_CENT),
    pire_cas: en_unites(IMAGE_MAX_NS, 100 - PART_DES_ENTREES_POUR_CENT),
    page: en_unites(IMAGE_COURANTE_NS, 100 - PART_DES_ENTREES_POUR_CENT) / MARGE_TELEPHONE,
};

// Une vue de pire cas a droit à l'image la plus longue ; une lecture courante, à une
// image de 60 par seconde : la séparation tient à la compilation.
const _: () = assert!(BUDGETS_E19.lecture < BUDGETS_E19.pire_cas);
const _: () = assert!(BUDGETS_E19.image_courante < BUDGETS_E19.image_max);
const _: () = assert!(BUDGETS_E19.page < BUDGETS_E19.lecture);

impl BudgetsE19 {
    /// Le coût fixe d'un appel d'un module dont le moteur remet `octets_remis_a_zero`
    /// octets à zéro avant chaque appel
    /// ([`moteur_wasmtime::ModuleCharge::octets_remis_a_zero`]), en unités de
    /// carburant : `(fixe + Kio × remise à zéro d'un Kio) / durée d'une unité`, le Kio
    /// entamé compté entier.
    pub const fn cout_fixe(&self, octets_remis_a_zero: u64) -> u64 {
        let kio = octets_remis_a_zero.div_ceil(1024);
        self.fixe_ns
            .saturating_mul(1000)
            .saturating_add(kio.saturating_mul(self.ps_par_kio_remis_a_zero))
            .div_ceil(self.ps_par_unite)
    }

    /// Une ligne pour le rapport : les budgets, dits provisoires, leur source, et le
    /// coût fixe d'un appel du module quand il est chargé.
    pub fn decrire(&self, octets_remis_a_zero: Option<u64>) -> String {
        let fixe = octets_remis_a_zero.map_or_else(
            || String::from("coût fixe d'un appel inconnu, le module n'étant pas chargé"),
            |o| {
                format!(
                    "coût fixe d'un appel de ce module {} unités ({o} octets remis à zéro par appel)",
                    self.cout_fixe(o)
                )
            },
        );
        format!(
            "provisoires, non fixés par le CONTRAT (bornes choisies au-dessus de mesures faites le 2026-11-04 dans wasmtime sur un Apple M5 : {} ps par unité, choisis, quand la mesure des appels lourds donne de 25 à 63 ps en médiane ; marge {} choisie pour le navigateur et l'ordinateur de la soirée ; la moitié de l'image au jeu) : entrées d'une image {} unités au plus, {} au 99ᵉ centile ; lecture courante {} au plus ; vue d'un pire cas {} au plus ; page du téléphone {} au plus (une lecture divisée par une marge de téléphone de {MARGE_TELEPHONE}, choisie, aucun téléphone n'étant mesuré) ; {fixe}",
            self.ps_par_unite,
            self.marge,
            self.image_max,
            self.image_courante,
            self.lecture,
            self.pire_cas,
            self.page,
        )
    }
}

/// Le retard de vue, en pas, au-delà duquel une arrivée vient d'un arriéré : sa seule
/// définition est dans le cœur, que l'épreuve relit.
pub const ARRIERE_PAS: u64 = partie::ARRIERE_PAS;

/// Les lectures que le cœur fait en jouant, jugées sur [`BudgetsE19::lecture`]. Les
/// autres (la forme canonique, la garde, `Bonjour`, `Decrire`) ne tombent pas dans
/// une image.
const LECTURES_JUGEES: [&str; 4] = ["Regarder", "Etat", "Valider", "Ressources"];

/// Un centile d'une suite triée : la plus petite valeur dont au moins `pour_cent` %
/// des valeurs sont plus petites ou égales.
pub(crate) fn centile(triees: &[u64], pour_cent: u64) -> u64 {
    let n = u64::try_from(triees.len()).unwrap_or(0);
    if n == 0 {
        return 0;
    }
    let rang = n.saturating_mul(pour_cent).div_ceil(100).max(1);
    usize::try_from(rang.saturating_sub(1))
        .ok()
        .and_then(|r| triees.get(r))
        .copied()
        .unwrap_or(0)
}

fn decrire(nom: &str, quoi: &str, v: &[u64]) -> String {
    format!(
        "{nom} : {} {quoi}, médiane {}, 99ᵉ centile {}, maximum {}",
        v.len(),
        centile(v, 50),
        centile(v, 99),
        v.last().copied().unwrap_or(0)
    )
}

/// Le coût d'un appel : son carburant, plus le coût fixe.
pub(crate) const fn cout(carburant: u64, fixe: u64) -> u64 {
    carburant.saturating_add(fixe)
}

/// Un tour de la boucle, pour E19 : le coût des entrées qu'il a jouées, le dernier pas
/// définitif après lui, et s'il a fini son avancée (sinon, un pas reste en cours).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TourJoue {
    pub(crate) cout: u64,
    pub(crate) definitif: u64,
    pub(crate) fini: bool,
}

/// L'image de chaque tour (voir « Ce que juge E19 ») : son coût et celui du tour
/// suivant, plus, s'il a fini son avancée, celui des entrées placées aux pas `s + 2` à
/// `s + fenetre + 1`, `s` son dernier pas définitif (`par_pas`, le coût des entrées par
/// pas) : le rejeu de la tête.
pub(crate) fn images(tours: &[TourJoue], par_pas: &BTreeMap<u64, u64>, fenetre: u64) -> Vec<u64> {
    tours
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let suivant = tours.get(i.saturating_add(1)).map_or(0, |u| u.cout);
            let tete = if t.fini && fenetre > 0 {
                let debut = t.definitif.saturating_add(2);
                let fin = t.definitif.saturating_add(fenetre).saturating_add(1);
                par_pas
                    .range(debut..=fin)
                    .fold(0u64, |a, (_, c)| a.saturating_add(*c))
            } else {
                0
            };
            t.cout.saturating_add(suivant).saturating_add(tete)
        })
        .collect()
}

/// Les images d'une suite de pas joués chacun d'un tour, sans découpage : la somme de
/// chaque suite de `largeur` pas consécutifs (la forme de la phase 2).
#[cfg(test)]
fn images_sans_decoupage(pas: &[u64], largeur: usize) -> Vec<u64> {
    let par_pas: BTreeMap<u64, u64> = (1u64..).zip(pas.iter().copied()).collect();
    // Le tour du pas `p` finit au pas `p` : son image compte les pas `p + 1` et suivants,
    // et son propre coût, celui du pas `p`.
    let tours: Vec<TourJoue> = (1u64..)
        .zip(pas.iter().copied())
        .map(|(p, cout)| TourJoue {
            cout,
            definitif: p,
            fini: true,
        })
        .collect();
    let fenetre = u64::try_from(largeur.saturating_sub(2)).unwrap_or(0);
    let mut v = images(&tours, &par_pas, fenetre);
    // Une fenêtre qui dépasse la soirée ne faisait pas d'image en phase 2.
    v.truncate(pas.len().saturating_sub(largeur.saturating_sub(1)));
    v
}

/// Juge les soirées : chaque lecture courante, le carburant de chaque appel plus
/// `fixe`, à son maximum ; chaque image (`(fenêtre + 2)` tours consécutifs, déjà
/// sommés par [`images`], fixes compris) à son maximum, et leur 99ᵉ centile.
pub(crate) fn juger(
    b: &BudgetsE19,
    appels: &BTreeMap<&'static str, Vec<u64>>,
    images: &[u64],
    fenetre: u64,
    fixe: u64,
) -> (Compte, Vec<String>) {
    let mut c = Compte::default();
    let mut notes = Vec::new();
    for (nom, v) in appels {
        let mut v = v.clone();
        v.sort_unstable();
        notes.push(format!("carburant, {}", decrire(nom, "appels", &v)));
        if LECTURES_JUGEES.contains(nom) {
            c.evaluer();
            let max = cout(v.last().copied().unwrap_or(0), fixe);
            if max > b.lecture {
                let trop = v.iter().filter(|x| cout(**x, fixe) > b.lecture).count();
                c.echouer(format!(
                    "{nom} : {trop} lecture(s) coûtent plus que le budget d'une lecture courante, {}, jusqu'à {max} (carburant et coût fixe de {fixe})",
                    b.lecture
                ));
            }
        }
    }
    let mut images = images.to_vec();
    images.sort_unstable();
    if !images.is_empty() {
        c.evaluer();
        notes.push(format!(
            "coût, fixes compris, {}",
            decrire(
                &format!("une image, (fenêtre {fenetre} + 2) tours"),
                "images",
                &images
            )
        ));
        let trop = images.iter().filter(|i| **i > b.image_max).count();
        if trop > 0 {
            let max = images.last().copied().unwrap_or(0);
            c.echouer(format!(
                "{trop} image(s) de (fenêtre {fenetre} + 2) tours consécutifs coûtent plus que le budget des entrées d'une image, {}, jusqu'à {max}",
                b.image_max
            ));
        }
        c.evaluer();
        let p99 = centile(&images, 99);
        if p99 > b.image_courante {
            c.echouer(format!(
                "le 99ᵉ centile du coût d'une image, {p99}, dépasse le budget des entrées d'une image courante (60 images par seconde), {}",
                b.image_courante
            ));
        }
    }
    (c, notes)
}

/// E19 sur les pires cas (E16) : la vue de l'écran de chaque pire cas, construit
/// depuis les limites déclarées, sous le budget d'une vue de pire cas, coût fixe
/// compris. Le carburant des soirées ne mesure que ce que les bots du jeu
/// atteignent ; un jeu qui ménage ses bots ne ménage pas ses pires cas.
pub(crate) fn juger_les_pires_cas(
    b: &BudgetsE19,
    vues: &[(u32, u64)],
    fixe: u64,
) -> (Compte, Vec<String>) {
    let mut c = Compte::default();
    let mut notes = Vec::new();
    for (indice, carburant) in vues {
        c.evaluer();
        let total = cout(*carburant, fixe);
        if total > b.pire_cas {
            c.echouer(format!(
                "la vue de l'écran du pire cas d'indice {indice} coûte {total} (carburant {carburant} et coût fixe {fixe}), au-delà du budget d'une vue de pire cas, {}",
                b.pire_cas
            ));
        }
    }
    if let Some(max) = vues.iter().map(|(_, c)| *c).max() {
        notes.push(format!(
            "pires cas : {} vues de l'écran, la plus lourde à {max} unités de carburant, {} avec le coût fixe",
            vues.len(),
            cout(max, fixe)
        ));
    }
    (c, notes)
}

/// E19 sur les pages du téléphone (`Page`, appelées par E21) : chacune sous
/// [`BudgetsE19::page`], coût fixe compris.
pub(crate) fn juger_les_pages(
    b: &BudgetsE19,
    carburants: &[u64],
    fixe: u64,
) -> (Compte, Vec<String>) {
    let mut c = Compte::default();
    let mut notes = Vec::new();
    if carburants.is_empty() {
        return (c, notes);
    }
    let mut v = carburants.to_vec();
    v.sort_unstable();
    notes.push(format!("carburant, {}", decrire("Page", "pages", &v)));
    c.evaluer();
    let max = cout(v.last().copied().unwrap_or(0), fixe);
    if max > b.page {
        let trop = v.iter().filter(|x| cout(**x, fixe) > b.page).count();
        c.echouer(format!(
            "Page : {trop} page(s) coûtent plus que le budget d'une page du téléphone, {} (une lecture courante divisée par {MARGE_TELEPHONE}), jusqu'à {max} (carburant et coût fixe de {fixe})",
            b.page
        ));
    }
    (c, notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const B: BudgetsE19 = BUDGETS_E19;

    /// PHASE-3, P7 : le retard de vue d'un arriéré n'a qu'une définition, celle du cœur.
    #[test]
    fn l_arriere_de_l_epreuve_est_celui_du_coeur() {
        assert_eq!(ARRIERE_PAS, partie::ARRIERE_PAS);
        assert_eq!(ARRIERE_PAS, 60, "la valeur de la phase 2, inchangée");
    }

    /// Revue de la phase 2, constat 10 : aucun chiffre n'est présenté comme mesuré s'il
    /// ne l'est pas. La durée d'une unité est une borne choisie : le rapport le dit.
    #[test]
    fn le_rapport_ne_dit_pas_mesure_une_borne_choisie() {
        let d = BUDGETS_E19.decrire(None);
        assert!(!d.contains("mesurés le"), "{d}");
        assert!(d.contains("200 ps par unité, choisis"), "{d}");
    }

    #[test]
    fn centiles() {
        let v: Vec<u64> = (1..=100).collect();
        assert_eq!(centile(&v, 99), 99);
        assert_eq!(centile(&v, 50), 50);
        assert_eq!(centile(&[7], 99), 7);
        assert_eq!(centile(&[], 99), 0);
    }

    /// La dérivation écrite dans la documentation du module : 33 ms × ½ × ½ /
    /// (0,2 ns × 4), et 1/60 s de même ; le coût fixe de `devine` (2 116 Kio de
    /// zones) et de 25 668 Kio.
    #[test]
    fn les_budgets_se_derivent_du_temps_d_une_image() {
        assert_eq!(B.image_max, 10_312_500);
        assert_eq!(B.pire_cas, 10_312_500);
        assert_eq!(B.image_courante, 5_208_333);
        assert_eq!(B.lecture, 5_208_333);
        assert_eq!(B.page, 1_302_083);
        assert_eq!(B.cout_fixe(2116 * 1024), 155_800);
        assert_eq!(B.cout_fixe(25_668 * 1024), 1_333_400);
        // Le Kio entamé compte entier ; sans zones, il reste le fixe.
        assert_eq!(B.cout_fixe(1), 50_050);
        assert_eq!(B.cout_fixe(0), 50_000);
        assert!(B.decrire(None).contains("provisoires"));
        assert!(B.decrire(Some(1024)).contains("50050 unités"));
    }

    /// RESTE-PHASE-1, point 18 : une lecture rare dix fois au-dessus du budget échoue,
    /// même sous le 99ᵉ centile ; une lecture juste au budget passe.
    #[test]
    fn une_lecture_rare_et_lourde_echoue() {
        let mut appels = BTreeMap::new();
        let mut v = vec![10u64; 999];
        v.push(B.lecture);
        let _: Option<Vec<u64>> = appels.insert("Regarder", v.clone());
        assert_eq!(juger(&B, &appels, &[], 0, 0).0.echecs, 0);
        // Le même appel, son coût fixe compris, dépasse.
        assert_eq!(juger(&B, &appels, &[], 0, 1).0.echecs, 1);
        v.push(10 * B.lecture);
        let _: Option<Vec<u64>> = appels.insert("Regarder", v);
        let (c, _) = juger(&B, &appels, &[], 0, 0);
        assert_eq!(c.echecs, 1, "{:?}", c.details);
        // Une sorte d'appel qui ne tombe pas dans une image n'est pas jugée.
        let mut autres = BTreeMap::new();
        let _: Option<Vec<u64>> = autres.insert("Canonique", vec![100 * B.pire_cas]);
        assert_eq!(juger(&B, &autres, &[], 0, 0).0.evaluations, 0);
    }

    /// CONTRAT, « aucune image au-delà de 33 ms en jeu » : une seule image au-delà du
    /// budget échoue, si rare soit-elle ; une image au budget passe.
    #[test]
    fn une_image_rare_et_lourde_echoue() {
        let mut pas = vec![10u64; 999];
        pas.push(B.image_max - 10);
        let (c, _) = juger(&B, &BTreeMap::new(), &images_sans_decoupage(&pas, 2), 0, 0);
        assert_eq!(c.echecs, 0, "{:?}", c.details);
        pas.push(11);
        let (c, _) = juger(&B, &BTreeMap::new(), &images_sans_decoupage(&pas, 2), 0, 0);
        assert_eq!((c.evaluations, c.echecs), (2, 1), "{:?}", c.details);
        assert_eq!(images_sans_decoupage(&[1, 2, 3], 2), vec![3, 5]);
        // Un tour qui laisse un pas inachevé ne compte que lui-même ; celui qui finit
        // compte aussi les pas qui suivent (PHASE-3, P7).
        let par_pas: BTreeMap<u64, u64> = [(5, 100), (6, 7), (7, 9)].into_iter().collect();
        let tours = [
            TourJoue {
                cout: 40,
                definitif: 4,
                fini: false,
            },
            TourJoue {
                cout: 60,
                definitif: 5,
                fini: true,
            },
        ];
        // Le premier : lui et le suivant, sans tête ; le second : lui, et la tête (le
        // pas 7, la fenêtre d'un pas après le pas qui suit).
        assert_eq!(images(&tours, &par_pas, 1), vec![100, 69]);
    }

    /// « Au moins 60 images par seconde » : un pour cent d'images au-delà du budget
    /// d'une image courante passe ; deux pour cent, non.
    #[test]
    fn le_99e_centile_juge_les_images_courantes() {
        let mut im = vec![10u64; 98];
        im.extend([B.image_courante + 1; 2]);
        assert_eq!(juger(&B, &BTreeMap::new(), &im, 0, 0).0.echecs, 1);
        let _: Option<u64> = im.pop();
        im.push(10);
        im.sort_unstable();
        assert_eq!(juger(&B, &BTreeMap::new(), &im, 0, 0).0.echecs, 0);
        // Une image est la somme de pas consécutifs : la fenêtre l'élargit.
        let pas = vec![B.image_courante / 4; 10];
        assert_eq!(
            juger(&B, &BTreeMap::new(), &images_sans_decoupage(&pas, 4), 2, 0)
                .0
                .echecs,
            0
        );
        assert_eq!(
            juger(&B, &BTreeMap::new(), &images_sans_decoupage(&pas, 5), 3, 0)
                .0
                .echecs,
            1
        );
    }

    /// PHASE-3, § 2.10 : une page du téléphone se juge à son maximum, coût fixe compris,
    /// sur une lecture divisée par la marge du téléphone ; aucune page, rien à juger.
    #[test]
    fn une_page_trop_lourde_pour_un_telephone_echoue() {
        let (c, _) = juger_les_pages(&B, &[10, B.page], 0);
        assert_eq!((c.evaluations, c.echecs), (1, 0), "{:?}", c.details);
        let (c, _) = juger_les_pages(&B, &[10, B.page], 1);
        assert_eq!((c.evaluations, c.echecs), (1, 1), "{:?}", c.details);
        let (c, _) = juger_les_pages(&B, &[], 0);
        assert_eq!(c.evaluations, 0);
        assert!(B.decrire(None).contains("page du téléphone 1302083"));
    }

    /// RESTE-PHASE-1, point 19 : un pire cas dont la vue dépasse son budget échoue,
    /// quoi que les bots du jeu aient choisi ; la vue d'un pire cas a droit à plus
    /// qu'une lecture courante, et son coût fixe compte.
    #[test]
    fn un_pire_cas_trop_lourd_echoue() {
        let (c, _) = juger_les_pires_cas(&B, &[(0, 10), (1, B.pire_cas + 1)], 0);
        assert_eq!((c.evaluations, c.echecs), (2, 1), "{:?}", c.details);
        let (c, _) = juger_les_pires_cas(&B, &[(0, B.lecture + 1)], 0);
        assert_eq!(c.echecs, 0, "{:?}", c.details);
        let (c, _) = juger_les_pires_cas(&B, &[(0, B.pire_cas)], 1);
        assert_eq!(c.echecs, 1, "{:?}", c.details);
    }
}
