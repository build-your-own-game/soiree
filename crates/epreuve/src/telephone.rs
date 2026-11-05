//! Un téléphone simulé et son lien avec l'ordinateur de la soirée (§ 7.1).
//!
//! Le transport simulé livre ce que le commun livrerait, sur un réseau qui ressemble
//! au réel (CONTRAT, « Ce que le réel impose » ; leçons SAL- et REM-) :
//!
//! - **un événement exactement une fois, dans l'ordre de l'appareil.** Un geste reste
//!   dans la file du téléphone jusqu'à ce que son pas soit définitif ; une coupure,
//!   un trou noir ou un redémarrage de l'autorité le font repartir. Le réseau peut le
//!   livrer deux fois : un accusé perdu dans une coupure le fait renvoyer, et
//!   l'ordinateur de la soirée reconnaît son identité (REM-13, SAL-15) ;
//! - **un état tenu ne vit que par son dernier** : il peut se perdre (WebRTC, canal
//!   non fiable), arriver dans le désordre ou par rafales, le numéro départage
//!   (REM-14). La page le redit toutes les 250 ms ([`RAPPEL_TENU`], REM-13) ;
//! - **deux canaux.** Un téléphone sur trois n'a que WebSocket : un paquet perdu y
//!   retient tous les suivants, dans les deux sens, le temps d'une retransmission
//!   ([`RETRANSMISSION`]). Les autres ont WebRTC : les gestes et les vues sur un canal
//!   fiable et ordonné (une perte y retient aussi les suivants), les tenus sur un
//!   canal qui perd et réordonne ;
//! - **avec latence, gigue et rafales de Wi-Fi** : 16 à 66 ms dans chaque sens, une
//!   gigue de deux pas, et, sur un téléphone qui en souffre, des paquets groupés par
//!   100 ms dans les deux sens ;
//! - **une horloge par téléphone** ([`Horloge`]) : un décalage, une dérive, et une
//!   estimation de l'heure de l'autorité qui se trompe d'environ un demi-aller-retour.
//!   `fait` porte cette erreur ; la demi-latence annoncée est celle que mesure l'écho,
//!   gigue comprise ;
//! - **la poignée de main peut échouer** : après une coupure ou un rechargement, une
//!   reconnexion sur deux en ratera une ou deux avant d'aboutir (SAL-85). Seule une
//!   poignée réussie fait annoncer le retour, et l'accueil réaligne les numéros des
//!   tenus (SAL-38) ;
//! - **un onglet rechargé repart de zéro** : ce qu'il n'avait pas envoyé est perdu,
//!   sa vue aussi, et il numérote ses tenus à partir de 1 jusqu'à l'accueil ;
//! - **un onglet en arrière-plan** se fige : il ne fait plus rien, relâche d'abord ce
//!   qu'il tenait (REM-11), ses vues s'accumulent et passent d'un coup à son retour, et
//!   le système ferme sa connexion au-delà de [`FERME_EN_ARRIERE_PLAN`] ;
//! - **des présences annoncées par le commun** : départ à la fermeture d'une
//!   connexion (après 0,5 s), ou après 3 s de silence ; retour, ou arrivée d'un
//!   appareil oublié (REM-43).
//!
//! Ces durées et ces proportions sont des points de départ, non mesurées.

use std::collections::BTreeMap;
use std::rc::Rc;

use jeu_abi::{Duree, Oui, StyleFil};
use moteur_wasmtime::InstantaneWasm;

use crate::tirage::Tirage;

/// Les pas avant que le commun voie une connexion fermée.
pub(crate) const FERMETURE: u64 = 30;
/// Les pas de silence avant que le commun annonce un départ (SQ-01).
pub(crate) const SILENCE_MAX: u64 = 180;
/// La rafale du Wi-Fi d'un téléphone, en pas : 100 ms.
pub(crate) const RAFALE: u64 = 6;
/// Le rappel du tenu, en pas : 250 ms (REM-13).
pub(crate) const RAPPEL_TENU: u64 = 15;
/// Une perte sur ce nombre de paquets d'un canal fiable (WebSocket, ou le canal
/// ordonné de WebRTC) : il est retransmis, et retient les suivants.
pub(crate) const PERTE_FIABLE: u64 = 40;
/// Une perte sur ce nombre de tenus du canal non fiable de WebRTC.
pub(crate) const PERTE_TENU: u64 = 25;
/// Le délai d'une retransmission, au moins : 200 ms, le plus petit RTO de TCP.
pub(crate) const RETRANSMISSION: u64 = 12;
/// La resynchronisation de l'horloge d'un téléphone, en pas : toutes les 5 s.
pub(crate) const SYNCHRO: u64 = 300;
/// La dérive d'une horloge, au plus, en millionièmes.
pub(crate) const DERIVE_MAX_PPM: u64 = 200;
/// Le décalage d'une horloge de téléphone, au plus, en pas : une minute.
pub(crate) const DECALAGE_MAX: u64 = 3600;
/// Au-delà de cette durée en arrière-plan, le système ferme la connexion : 5 s.
pub(crate) const FERME_EN_ARRIERE_PLAN: u64 = 300;
/// Une fraction de pas : 1/65 536.
const FRACTIONS: i64 = 1 << 16;

/// L'horloge d'un téléphone (CONTRAT : « Il n'existe pas de “maintenant” partagé »).
///
/// Le téléphone lit son horloge locale, décalée et qui dérive. Le commun estime
/// l'écart avec l'heure de l'autorité par un aller-retour ([`Horloge::synchroniser`]) :
/// l'estimation se trompe de l'asymétrie inconnue du chemin, jusqu'à un
/// demi-aller-retour, puis la dérive s'ajoute jusqu'à la synchronisation suivante.
/// Le décalage, lui, s'annule : il ne sert qu'à faire de l'horloge locale une vraie
/// horloge. Tout se compte en 1/65 536 de pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Horloge {
    /// Le décalage de l'horloge locale à l'instant zéro de la partie.
    decalage: i64,
    /// Sa dérive, en millionièmes, signée.
    derive_ppm: i64,
    /// L'écart estimé entre l'horloge locale et l'heure de l'autorité.
    ecart_estime: i64,
    /// La demi-latence que l'écho a mesurée, en pas : celle que l'arrivée annonce.
    pub(crate) demi: u64,
    /// La dernière synchronisation, en pas de l'autorité.
    pub(crate) synchro: u64,
}

impl Horloge {
    /// Une horloge exacte, sans erreur : le monde idéal de la phase 1.
    pub(crate) const fn exacte(latence: u64) -> Horloge {
        Horloge {
            decalage: 0,
            derive_ppm: 0,
            ecart_estime: 0,
            demi: latence,
            synchro: 0,
        }
    }

    /// Une horloge réelle : un décalage et une dérive tirés.
    pub(crate) fn tiree(t: &mut Tirage, latence: u64) -> Horloge {
        let signe = |t: &mut Tirage, v: u64| {
            let v = i64::try_from(v).unwrap_or(0);
            if t.une_sur(2) { v } else { v.saturating_neg() }
        };
        let d = t.sous(DECALAGE_MAX.saturating_mul(1 << 16));
        let decalage = signe(t, d);
        let p = t.entre(0, DERIVE_MAX_PPM);
        let derive_ppm = signe(t, p);
        Horloge {
            decalage,
            derive_ppm,
            ecart_estime: decalage,
            demi: latence,
            synchro: 0,
        }
    }

    /// L'horloge locale à l'instant `vrai` de l'autorité (en fractions).
    fn locale(&self, vrai: i64) -> i64 {
        let derive = vrai.saturating_mul(self.derive_ppm) / 1_000_000;
        vrai.saturating_add(derive).saturating_add(self.decalage)
    }

    /// Une synchronisation par un aller-retour de `aller` puis `retour` pas, faite au
    /// pas `pas` de l'autorité. L'estimation suppose le chemin symétrique : elle se
    /// trompe de la moitié de l'asymétrie, que `asymetrie` tire entre −1 et 1 (en
    /// 1/65 536) : l'erreur va jusqu'à un demi-aller-retour.
    pub(crate) fn synchroniser(&mut self, pas: u64, aller: u64, retour: u64, asymetrie: i64) {
        let rtt = aller.saturating_add(retour);
        let demi_rtt = i64::try_from(rtt).unwrap_or(0).saturating_mul(FRACTIONS) / 2;
        let erreur = demi_rtt.saturating_mul(asymetrie.clamp(-FRACTIONS, FRACTIONS)) / FRACTIONS;
        let vrai = i64::try_from(pas)
            .unwrap_or(i64::MAX)
            .saturating_mul(FRACTIONS);
        self.ecart_estime = self
            .locale(vrai)
            .saturating_sub(vrai)
            .saturating_sub(erreur);
        self.demi = rtt.div_ceil(2);
        self.synchro = pas;
    }

    /// L'heure de l'autorité que le téléphone estime à l'instant vrai `(pas,
    /// fraction)` : ce qu'il annonce dans `fait`.
    pub(crate) fn estimer(&self, (pas, fraction): (u64, u16)) -> (u64, u16) {
        let vrai = i64::try_from(pas)
            .unwrap_or(i64::MAX)
            .saturating_mul(FRACTIONS)
            .saturating_add(i64::from(fraction));
        let e = self.locale(vrai).saturating_sub(self.ecart_estime).max(0);
        let pas = u64::try_from(e / FRACTIONS).unwrap_or(0);
        let fraction = u16::try_from(e % FRACTIONS).unwrap_or(0);
        (pas, fraction)
    }

    /// La demi-latence mesurée.
    pub(crate) const fn demi_aller_retour(&self) -> Duree {
        Duree::pas(self.demi)
    }
}

/// Le canal d'un téléphone (CONTRAT, « Le canal idéal n'existe pas partout »).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Canal {
    /// Un canal fiable et ordonné pour les gestes et les vues, un canal qui perd et
    /// réordonne pour les tenus.
    WebRtc,
    /// Tout sur une seule file fiable et ordonnée : une perte retient tout le reste.
    WebSocket,
}

/// L'état du lien d'un téléphone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lien {
    /// Pas encore arrivé, ou parti pour de bon.
    Hors,
    /// Tout passe.
    Ok,
    /// La connexion est fermée jusqu'à ce pas.
    Coupe { fin: u64 },
    /// Le téléphone dort jusqu'à ce pas, connexion ouverte.
    Silence { fin: u64 },
    /// Ce qu'il envoie se perd jusqu'à ce pas.
    TrouNoir { fin: u64 },
    /// Ses paquets sont retenus jusqu'à ce pas.
    Retenu { fin: u64 },
    /// L'onglet se recharge jusqu'à ce pas.
    Recharge { fin: u64 },
    /// Une poignée de main jusqu'à ce pas : réussie si `ratees` vaut 0, sinon elle
    /// échoue et la suivante commence.
    Poignee { fin: u64, ratees: u8 },
    /// L'onglet est en arrière-plan jusqu'à ce pas : figé.
    ArrierePlan { fin: u64 },
}

impl Lien {
    /// Le téléphone peut-il agir (son écran est allumé, sa page vit, elle est au
    /// premier plan) ?
    pub(crate) const fn agit(self) -> bool {
        matches!(
            self,
            Lien::Ok
                | Lien::Coupe { .. }
                | Lien::TrouNoir { .. }
                | Lien::Retenu { .. }
                | Lien::Poignee { .. }
        )
    }

    /// Ses paquets partent-ils sur le réseau ?
    pub(crate) const fn envoie(self) -> bool {
        matches!(self, Lien::Ok | Lien::Retenu { .. })
    }

    /// Sa connexion est-elle ouverte, pour recevoir des vues ? Une page figée les
    /// reçoit dans sa connexion sans les lire.
    pub(crate) const fn recoit(self) -> bool {
        matches!(
            self,
            Lien::Ok | Lien::Retenu { .. } | Lien::Silence { .. } | Lien::ArrierePlan { .. }
        )
    }

    /// La page lit-elle les vues qui lui arrivent ? Figée, elle les laisse dans la
    /// connexion, et les lit d'un coup à son réveil.
    pub(crate) const fn lit(self) -> bool {
        !matches!(self, Lien::Silence { .. } | Lien::ArrierePlan { .. })
    }
}

/// Où en est un événement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EtatEvenement {
    /// À envoyer dès que le lien le permet.
    AEnvoyer,
    /// Sur le réseau.
    EnVol,
    /// Placé par le cœur à ce pas, pas encore définitif ; l'accusé arrive au pas
    /// `accuse`.
    Place { pas: u64, accuse: u64 },
}

/// Un événement d'un téléphone : un geste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Evenement {
    pub(crate) id: u64,
    pub(crate) octets: Vec<u8>,
    /// L'instant annoncé, sur l'horloge estimée du téléphone.
    pub(crate) fait: (u64, u16),
    /// Le pas de la vue que la page montrait quand le geste a été fait : il part avec
    /// lui, même renvoyé bien plus tard.
    pub(crate) vu: u64,
    pub(crate) etat: EtatEvenement,
    /// Vient-il de l'onglet jumeau ?
    pub(crate) jumeau: Oui,
}

/// Ce qui voyage sur le réseau, du téléphone vers l'ordinateur de la soirée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Paquet {
    /// Un geste, par son identité.
    Geste { id: u64 },
    /// Un état tenu.
    Tenu {
        octets: Vec<u8>,
        numero: u64,
        fait: (u64, u16),
    },
}

/// Un paquet en route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnRoute {
    pub(crate) livre_a: u64,
    pub(crate) paquet: Paquet,
}

/// Une vue en route vers le téléphone, avec l'état de la définitive d'où elle vient :
/// le bot décidera dessus (point 20).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VueEnRoute {
    pub(crate) livre_a: u64,
    pub(crate) pas: u64,
    pub(crate) octets: Vec<u8>,
    pub(crate) etat: Option<Rc<InstantaneWasm>>,
}

/// Ce que le réseau d'un téléphone a fait, pour les tests et le rapport.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CompteReseau {
    /// Paquets perdus puis retransmis, qui ont retenu les suivants.
    pub(crate) retransmis: u64,
    /// Tenus perdus sur le canal non fiable.
    pub(crate) tenus_perdus: u64,
    /// Gestes renvoyés après un accusé perdu, reconnus à leur arrivée.
    pub(crate) doubles: u64,
    /// Poignées de main ratées.
    pub(crate) poignees_ratees: u64,
    /// Accueils où la page numérotait sous le dernier numéro reçu : réalignée.
    pub(crate) realignements: u64,
    /// Rappels du tenu.
    pub(crate) rappels: u64,
    /// Vues lues d'un coup au retour du premier plan, ou d'un sommeil.
    pub(crate) vues_en_rafale: u64,
}

/// Un téléphone simulé.
#[derive(Debug, Clone)]
pub(crate) struct Telephone {
    pub(crate) appareil: u64,
    pub(crate) style: StyleFil,
    /// Le pas de sa première connexion.
    pub(crate) arrive: u64,
    /// Le pas où il part pour de bon, s'il part.
    pub(crate) part: Option<u64>,
    pub(crate) lien: Lien,
    /// Le jeu le connaît-il (arrivé, pas oublié) ?
    pub(crate) connu: Oui,
    /// Le commun l'a-t-il annoncé présent ?
    pub(crate) present: Oui,
    /// Sa latence dans chaque sens, en pas.
    pub(crate) latence: u64,
    pub(crate) rafales: Oui,
    pub(crate) canal: Canal,
    pub(crate) horloge: Horloge,
    /// Son hasard : gigue, pertes, poignées de main.
    pub(crate) hasard: Tirage,
    /// Le dernier paquet de la file fiable vers l'ordinateur de la soirée : rien ne
    /// passe avant lui.
    pub(crate) file_montante: u64,
    /// … et vers le téléphone.
    pub(crate) file_descendante: u64,
    /// Son onglet jumeau joue jusqu'à ce pas.
    pub(crate) jumeau_jusqua: u64,
    /// `Panne::Doublon` : le réseau livre deux fois chaque geste qui part, tant
    /// qu'aucun n'a été reçu deux fois.
    pub(crate) doubler: Oui,
    pub(crate) reseau: Vec<EnRoute>,
    pub(crate) evenements: Vec<Evenement>,
    /// Le dernier état tenu, à renvoyer à chaque reconnexion et à chaque rappel.
    pub(crate) tenu: Option<(Vec<u8>, (u64, u16))>,
    pub(crate) prochain_id: u64,
    pub(crate) numero_tenu: u64,
    /// Le prochain rappel du tenu.
    pub(crate) prochain_rappel: u64,
    /// La dernière livraison d'un geste : l'ordre de l'appareil se garde.
    pub(crate) dernier_geste_livre: u64,
    /// Le pas de la vue qu'il montre ; 0 tant qu'il n'en a reçu aucune.
    pub(crate) vu: u64,
    /// Ses octets.
    pub(crate) vue: Vec<u8>,
    /// L'état de la définitive d'où vient la vue qu'il montre.
    pub(crate) etat_vu: Option<Rc<InstantaneWasm>>,
    /// Le pas où elle a changé.
    pub(crate) vue_depuis: u64,
    pub(crate) vues: Vec<VueEnRoute>,
    /// Le prochain pas où son bot joue.
    pub(crate) prochain_bot: u64,
    /// La dernière vue envoyée, pour ne renvoyer que ce qui change.
    pub(crate) envoyee: Vec<u8>,
    /// Ce que l'ordinateur de la soirée a reçu de ses gestes, par identité, avec leur
    /// place : un geste renvoyé n'y entre pas deux fois.
    pub(crate) recus: BTreeMap<u64, u64>,
    pub(crate) compte: CompteReseau,
}

impl Telephone {
    pub(crate) fn new(
        appareil: u64,
        style: StyleFil,
        arrive: u64,
        latence: u64,
        mut hasard: Tirage,
        horloges: Oui,
    ) -> Telephone {
        let canal = if hasard.une_sur(3) {
            Canal::WebSocket
        } else {
            Canal::WebRtc
        };
        let horloge = match horloges {
            Oui::Oui => Horloge::tiree(&mut hasard, latence),
            Oui::Non => Horloge::exacte(latence),
        };
        Telephone {
            appareil,
            style,
            arrive,
            part: None,
            lien: Lien::Hors,
            connu: Oui::Non,
            present: Oui::Non,
            latence,
            rafales: Oui::Non,
            canal,
            horloge,
            hasard,
            file_montante: 0,
            file_descendante: 0,
            jumeau_jusqua: 0,
            doubler: Oui::Non,
            reseau: Vec::new(),
            evenements: Vec::new(),
            tenu: None,
            prochain_id: 1,
            numero_tenu: 0,
            prochain_rappel: 0,
            dernier_geste_livre: 0,
            vu: 0,
            vue: Vec::new(),
            etat_vu: None,
            vue_depuis: 0,
            vues: Vec::new(),
            prochain_bot: arrive,
            envoyee: Vec::new(),
            recus: BTreeMap::new(),
            compte: CompteReseau::default(),
        }
    }

    /// Un sens du trajet commencé à `maintenant` : latence, gigue, rafales, rétention.
    fn trajet(&mut self, maintenant: u64) -> u64 {
        let gigue = self.hasard.sous(3);
        let mut a = maintenant
            .saturating_add(self.latence)
            .saturating_add(gigue);
        if self.rafales == Oui::Oui {
            let reste = a.checked_rem(RAFALE).unwrap_or(0);
            if reste != 0 {
                a = a.saturating_add(RAFALE.saturating_sub(reste));
            }
        }
        if let Lien::Retenu { fin } = self.lien {
            a = a.max(fin);
        }
        a
    }

    /// Une perte sur un canal fiable : le paquet repart après une retransmission.
    fn perte_fiable(&mut self, a: u64) -> u64 {
        if self.hasard.une_sur(PERTE_FIABLE) {
            self.compte.retransmis = self.compte.retransmis.saturating_add(1);
            let rto = RETRANSMISSION.saturating_add(self.hasard.sous(RETRANSMISSION));
            a.saturating_add(rto)
        } else {
            a
        }
    }

    /// Le pas où un paquet de la file fiable montante, envoyé maintenant, arrive.
    fn montee_fiable(&mut self, maintenant: u64) -> u64 {
        let a = self.trajet(maintenant);
        let a = self.perte_fiable(a).max(self.file_montante);
        self.file_montante = a;
        a
    }

    /// Le pas où une vue envoyée maintenant arrive au téléphone : sur la file fiable
    /// descendante, qu'une perte retient (les deux canaux la portent ainsi).
    pub(crate) fn livraison_de_vue(&mut self, maintenant: u64) -> u64 {
        let a = self.trajet(maintenant);
        let a = self.perte_fiable(a).max(self.file_descendante);
        self.file_descendante = a;
        a
    }

    /// Un geste neuf : il entre dans la file, et part si le lien le permet. `fait` est
    /// l'instant vrai : le téléphone annonce son estimation.
    pub(crate) fn emettre(
        &mut self,
        octets: Vec<u8>,
        fait: (u64, u16),
        jumeau: Oui,
        maintenant: u64,
    ) {
        let base = self.prochain_id;
        self.prochain_id = base.saturating_add(1);
        let id = match jumeau {
            Oui::Oui => base | (1 << 62),
            Oui::Non => base,
        };
        self.evenements.push(Evenement {
            id,
            octets,
            fait: self.horloge.estimer(fait),
            vu: self.vu,
            etat: EtatEvenement::AEnvoyer,
            jumeau,
        });
        self.envoyer_les_gestes(maintenant);
    }

    /// Envoie les gestes qui attendent, dans leur ordre, si le lien le permet.
    pub(crate) fn envoyer_les_gestes(&mut self, maintenant: u64) {
        if !self.lien.envoie() {
            return;
        }
        let ids: Vec<u64> = self
            .evenements
            .iter()
            .filter(|e| e.etat == EtatEvenement::AEnvoyer)
            .map(|e| e.id)
            .collect();
        for id in ids {
            let a = match self.canal {
                Canal::WebSocket => self.montee_fiable(maintenant),
                Canal::WebRtc => {
                    let a = self.trajet(maintenant);
                    self.perte_fiable(a)
                }
            }
            .max(self.dernier_geste_livre);
            self.dernier_geste_livre = a;
            if let Some(e) = self.evenements.iter_mut().find(|e| e.id == id) {
                e.etat = EtatEvenement::EnVol;
            }
            self.reseau.push(EnRoute {
                livre_a: a,
                paquet: Paquet::Geste { id },
            });
            if self.doubler == Oui::Oui {
                // La copie suit l'original de un à trois pas.
                let ecart = 1u64.saturating_add(self.hasard.sous(3));
                self.reseau.push(EnRoute {
                    livre_a: a.saturating_add(ecart),
                    paquet: Paquet::Geste { id },
                });
            }
        }
    }

    /// Un état tenu neuf : le dernier gagne ; il part s'il le peut, sinon il attend
    /// la reconnexion.
    pub(crate) fn tenir(&mut self, octets: Vec<u8>, fait: (u64, u16), maintenant: u64) {
        self.tenu = Some((octets, fait));
        self.prochain_rappel = maintenant.saturating_add(RAPPEL_TENU);
        self.envoyer_le_tenu(maintenant);
    }

    /// Le rappel régulier du tenu (REM-13) : la page redit ce qu'elle tient, faite
    /// maintenant, toutes les [`RAPPEL_TENU`] pas.
    pub(crate) fn rappeler(&mut self, maintenant: u64) {
        if maintenant < self.prochain_rappel || !self.lien.envoie() || self.tenu.is_none() {
            return;
        }
        if let Some((_, fait)) = self.tenu.as_mut() {
            *fait = (maintenant, 0);
        }
        self.prochain_rappel = maintenant.saturating_add(RAPPEL_TENU);
        self.compte.rappels = self.compte.rappels.saturating_add(1);
        self.envoyer_le_tenu(maintenant);
    }

    /// Renvoie l'état tenu, avec un numéro neuf.
    pub(crate) fn envoyer_le_tenu(&mut self, maintenant: u64) {
        if !self.lien.envoie() {
            return;
        }
        let Some((octets, fait)) = self.tenu.clone() else {
            return;
        };
        self.numero_tenu = self.numero_tenu.saturating_add(1);
        let livre_a = match self.canal {
            Canal::WebSocket => self.montee_fiable(maintenant),
            Canal::WebRtc => {
                if self.hasard.une_sur(PERTE_TENU) {
                    self.compte.tenus_perdus = self.compte.tenus_perdus.saturating_add(1);
                    return;
                }
                self.trajet(maintenant)
            }
        };
        let fait = self.horloge.estimer(fait);
        self.reseau.push(EnRoute {
            livre_a,
            paquet: Paquet::Tenu {
                octets,
                numero: self.numero_tenu,
                fait,
            },
        });
    }

    /// Le réseau perd tout ce qui est en route : les gestes repartiront. Un accusé
    /// encore en route se perd aussi : son geste repartira, et l'ordinateur de la
    /// soirée le reconnaîtra (un geste reçu deux fois).
    pub(crate) fn perdre_le_reseau(&mut self, maintenant: u64) {
        self.reseau.clear();
        self.file_montante = 0;
        self.file_descendante = 0;
        self.dernier_geste_livre = 0;
        for e in &mut self.evenements {
            match e.etat {
                EtatEvenement::EnVol => e.etat = EtatEvenement::AEnvoyer,
                EtatEvenement::Place { accuse, .. } if accuse > maintenant => {
                    e.etat = EtatEvenement::AEnvoyer;
                }
                EtatEvenement::Place { .. } | EtatEvenement::AEnvoyer => {}
            }
        }
    }

    /// L'onglet se recharge : ce qu'il n'avait pas envoyé meurt avec lui, son état
    /// tenu, sa vue et sa numérotation aussi, qui repart de zéro (SAL-38).
    pub(crate) fn recharger(&mut self, fin: u64, maintenant: u64) {
        self.lien = Lien::Recharge { fin };
        self.perdre_le_reseau(maintenant);
        self.evenements
            .retain(|e| matches!(e.etat, EtatEvenement::Place { .. }));
        self.tenu = None;
        self.numero_tenu = 0;
        self.vue.clear();
        self.vu = 0;
        self.etat_vu = None;
        self.envoyee.clear();
        self.vues.clear();
    }

    /// Une reconnexion commence : une poignée de main, ratée une fois sur trois, deux
    /// fois sur six (SAL-85). Rend vrai si elle aboutit tout de suite, sans poignée
    /// ratée.
    pub(crate) fn reconnecter(&mut self, maintenant: u64) -> bool {
        let ratees = match self.hasard.sous(6) {
            0..=2 => 0,
            3 | 4 => 1,
            _ => 2,
        };
        if ratees == 0 {
            self.lien = Lien::Ok;
            return true;
        }
        self.lien = Lien::Poignee {
            fin: self.fin_de_poignee(maintenant),
            ratees,
        };
        false
    }

    /// La fin d'une poignée de main ratée : le temps d'un aller-retour et d'un peu
    /// plus, puis l'attente avant le nouvel essai.
    fn fin_de_poignee(&mut self, maintenant: u64) -> u64 {
        let essai = self.latence.saturating_mul(2).saturating_add(3);
        let attente = self.hasard.entre(30, 90);
        maintenant.saturating_add(essai).saturating_add(attente)
    }

    /// Au pas `maintenant`, une poignée de main en cours avance. Rend vrai si la
    /// connexion vient d'aboutir.
    pub(crate) fn poignee_du_pas(&mut self, maintenant: u64) -> bool {
        let Lien::Poignee { fin, ratees } = self.lien else {
            return false;
        };
        if maintenant < fin {
            return false;
        }
        self.compte.poignees_ratees = self.compte.poignees_ratees.saturating_add(1);
        if ratees <= 1 {
            self.lien = Lien::Ok;
            return true;
        }
        self.lien = Lien::Poignee {
            fin: self.fin_de_poignee(maintenant),
            ratees: ratees.saturating_sub(1),
        };
        false
    }

    /// L'accueil d'une connexion réussie : la page reprend sa numérotation au-dessus du
    /// plus grand numéro que l'ordinateur de la soirée a reçu d'elle (SAL-38).
    pub(crate) fn accueillir(&mut self, dernier: Option<u64>) {
        if let Some(n) = dernier
            && n >= self.numero_tenu
        {
            if self.numero_tenu < n {
                self.compte.realignements = self.compte.realignements.saturating_add(1);
            }
            self.numero_tenu = n;
        }
    }

    /// Une resynchronisation de l'horloge, si elle est due : un aller-retour réel, sur
    /// le chemin de ce moment (gigue et rafales), et une asymétrie inconnue.
    pub(crate) fn synchroniser_si_du(&mut self, maintenant: u64, reel: Oui) {
        if reel == Oui::Non
            || self.lien != Lien::Ok
            || maintenant < self.horloge.synchro.saturating_add(SYNCHRO)
        {
            return;
        }
        let aller = self.trajet(maintenant).saturating_sub(maintenant);
        let retour = self
            .trajet(maintenant.saturating_add(aller))
            .saturating_sub(maintenant.saturating_add(aller));
        let n = i64::try_from(self.hasard.sous(1 << 17)).unwrap_or(0);
        self.horloge
            .synchroniser(maintenant, aller, retour, n.saturating_sub(FRACTIONS));
    }

    /// Les paquets qui arrivent à ce pas, dans leur ordre d'arrivée.
    pub(crate) fn arrivent(&mut self, maintenant: u64) -> Vec<Paquet> {
        let mut arrives: Vec<EnRoute> = Vec::new();
        let mut restent = Vec::new();
        for p in std::mem::take(&mut self.reseau) {
            if p.livre_a <= maintenant {
                arrives.push(p);
            } else {
                restent.push(p);
            }
        }
        self.reseau = restent;
        arrives.sort_by_key(|p| p.livre_a);
        arrives.into_iter().map(|p| p.paquet).collect()
    }

    /// Les gestes confirmés par un pas définitif quittent la file, quand leur accusé est
    /// arrivé : jusque-là, le téléphone ne sait pas qu'ils sont passés, et une coupure
    /// les fait renvoyer.
    pub(crate) fn confirmer(&mut self, definitif: u64, maintenant: u64) {
        self.evenements.retain(|e| {
            !matches!(e.etat, EtatEvenement::Place { pas, accuse }
                if pas <= definitif && accuse <= maintenant)
        });
        // Un geste qui a quitté la file ne repart plus : l'ordinateur de la soirée n'a
        // plus à le reconnaître.
        let restent: std::collections::BTreeSet<u64> =
            self.evenements.iter().map(|e| e.id).collect();
        self.recus.retain(|id, _| restent.contains(id));
    }

    /// Après un redémarrage de l'autorité : ce qui n'était pas définitif repart, et
    /// l'autorité neuve l'oublie (elle ne le retrouve pas au journal).
    pub(crate) fn renvoyer_le_provisoire(&mut self, definitif: u64) {
        self.recus.retain(|_, place| *place <= definitif);
        for e in &mut self.evenements {
            if matches!(e.etat, EtatEvenement::Place { pas, .. } if pas > definitif) {
                e.etat = EtatEvenement::AEnvoyer;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use jeu_abi::{Oui, StyleFil};

    use super::{
        Canal, DERIVE_MAX_PPM, EtatEvenement, Horloge, Lien, Paquet, RAPPEL_TENU, SYNCHRO,
        Telephone,
    };
    use crate::tirage::Tirage;

    fn telephone(graine: u8) -> Telephone {
        let mut t = Telephone::new(
            7,
            StyleFil::Sage,
            0,
            3,
            Tirage::new(&[graine; 32], "téléphone"),
            Oui::Oui,
        );
        t.lien = Lien::Ok;
        t
    }

    /// CONTRAT, « Il n'existe pas de “maintenant” partagé » : l'horloge estimée se
    /// trompe d'au plus un demi-aller-retour, plus la dérive depuis la synchronisation,
    /// et elle se trompe vraiment, dans les deux sens.
    #[test]
    fn l_heure_estimee_se_trompe_d_un_demi_aller_retour() {
        let mut avant = 0;
        let mut apres = 0;
        for g in 0..64u8 {
            let mut t = telephone(g);
            t.synchroniser_si_du(SYNCHRO, Oui::Oui);
            let demi = i64::try_from(t.horloge.demi).unwrap() * 65_536;
            for d in [0u64, 100, SYNCHRO - 1] {
                let vrai = SYNCHRO + d;
                let (p, f) = t.horloge.estimer((vrai, 0));
                let e = i64::try_from(p).unwrap() * 65_536 + i64::from(f)
                    - i64::try_from(vrai).unwrap() * 65_536;
                let derive = i64::try_from(d * DERIVE_MAX_PPM * 65_536 / 1_000_000).unwrap() + 1;
                assert!(e.abs() <= demi + derive, "{e} > {demi} + {derive}");
                if e < 0 {
                    avant += 1;
                }
                if e > 0 {
                    apres += 1;
                }
            }
        }
        assert!(
            avant > 0 && apres > 0,
            "{avant} en avance, {apres} en retard"
        );
        let exacte = Horloge::exacte(2);
        assert_eq!(exacte.estimer((40, 9)), (40, 9));
    }

    /// Un geste porte l'heure estimée et la vue d'où il part, même renvoyé plus tard.
    #[test]
    fn un_geste_part_avec_sa_vue_et_son_heure_estimee() {
        let mut t = telephone(1);
        t.synchroniser_si_du(SYNCHRO, Oui::Oui);
        t.vu = 290;
        t.emettre(vec![1], (SYNCHRO + 5, 0), Oui::Non, SYNCHRO + 5);
        t.vu = 400;
        let e = &t.evenements[0];
        assert_eq!(e.vu, 290);
        assert_eq!(e.fait, t.horloge.estimer((SYNCHRO + 5, 0)));
    }

    /// WebSocket : un paquet perdu retient tous les suivants, jusqu'à sa retransmission.
    #[test]
    fn sur_websocket_une_perte_retient_la_file() {
        let mut vu = false;
        for g in 0..32u8 {
            let mut t = telephone(g);
            t.canal = Canal::WebSocket;
            for p in 0..400 {
                t.tenir(vec![u8::try_from(p % 200).unwrap()], (p, 0), p);
            }
            let dates: Vec<u64> = t.reseau.iter().map(|r| r.livre_a).collect();
            assert!(dates.windows(2).all(|w| w[0] <= w[1]), "ordonnée");
            if t.compte.retransmis > 0 {
                // Une retransmission tient la file : des paquets envoyés à des pas
                // différents arrivent au même pas, derrière elle.
                assert!(dates.windows(2).any(|w| w[0] == w[1]));
                vu = true;
            }
        }
        assert!(vu);
    }

    /// WebRTC : les tenus se perdent et se réordonnent ; les gestes restent dans
    /// l'ordre.
    #[test]
    fn sur_webrtc_les_tenus_se_perdent_et_se_reordonnent() {
        let mut t = telephone(5);
        t.canal = Canal::WebRtc;
        for p in 0..600 {
            t.tenir(vec![1], (p, 0), p);
            t.emettre(vec![2], (p, 0), Oui::Non, p);
        }
        assert!(t.compte.tenus_perdus > 0);
        let tenus: Vec<(u64, u64)> = t
            .reseau
            .iter()
            .filter_map(|r| match &r.paquet {
                Paquet::Tenu { numero, .. } => Some((*numero, r.livre_a)),
                Paquet::Geste { .. } => None,
            })
            .collect();
        assert!(tenus.windows(2).any(|w| w[1].1 < w[0].1), "réordonnés");
        let gestes: Vec<u64> = t
            .reseau
            .iter()
            .filter(|r| matches!(r.paquet, Paquet::Geste { .. }))
            .map(|r| r.livre_a)
            .collect();
        assert!(gestes.windows(2).all(|w| w[0] <= w[1]));
    }

    /// REM-13 : la page redit son tenu toutes les 250 ms, faite maintenant.
    #[test]
    fn le_tenu_se_redit_regulierement() {
        let mut t = telephone(2);
        t.canal = Canal::WebSocket;
        t.tenir(vec![9], (10, 0), 10);
        for p in 11..=10 + 4 * RAPPEL_TENU {
            t.rappeler(p);
        }
        assert_eq!(t.compte.rappels, 4);
        let numeros: Vec<u64> = t
            .reseau
            .iter()
            .filter_map(|r| match &r.paquet {
                Paquet::Tenu { numero, .. } => Some(*numero),
                Paquet::Geste { .. } => None,
            })
            .collect();
        assert_eq!(numeros, [1, 2, 3, 4, 5]);
    }

    /// SAL-38 : un onglet rechargé repart de zéro ; l'accueil le réaligne au-dessus du
    /// dernier numéro reçu.
    #[test]
    fn un_onglet_recharge_repart_de_zero_puis_l_accueil_le_realigne() {
        let mut t = telephone(3);
        t.canal = Canal::WebSocket;
        for p in 0..5 {
            t.tenir(vec![1], (p, 0), p);
        }
        assert_eq!(t.numero_tenu, 5);
        t.recharger(100, 5);
        assert_eq!((t.numero_tenu, t.vu, t.tenu.clone()), (0, 0, None));
        t.lien = Lien::Ok;
        t.accueillir(Some(5));
        assert_eq!(t.compte.realignements, 1);
        t.tenir(vec![2], (110, 0), 110);
        assert!(
            t.reseau
                .iter()
                .any(|r| matches!(r.paquet, Paquet::Tenu { numero: 6, .. }))
        );
    }

    /// SAL-85 : une reconnexion rate parfois sa poignée de main, une ou deux fois,
    /// avant d'aboutir.
    #[test]
    fn la_poignee_de_main_rate_parfois() {
        let mut ratees = 0;
        let mut directes = 0;
        for g in 0..32u8 {
            let mut t = telephone(g);
            if t.reconnecter(0) {
                directes += 1;
                continue;
            }
            let mut p = 0;
            while !t.poignee_du_pas(p) {
                assert!(matches!(t.lien, Lien::Poignee { .. }));
                assert!(!t.lien.envoie());
                p += 1;
                assert!(p < 1000);
            }
            assert_eq!(t.lien, Lien::Ok);
            ratees += t.compte.poignees_ratees;
        }
        assert!(ratees > 0 && directes > 0);
    }

    /// Un geste définitif ne quitte la file qu'à l'arrivée de son accusé.
    #[test]
    fn un_geste_ne_quitte_la_file_qu_a_son_accuse() {
        let mut t = telephone(6);
        t.emettre(vec![1], (0, 0), Oui::Non, 0);
        let id = t.evenements[0].id;
        t.evenements[0].etat = EtatEvenement::Place { pas: 4, accuse: 8 };
        let _: Option<u64> = t.recus.insert(id, 4);
        t.confirmer(5, 6);
        assert_eq!(t.evenements.len(), 1, "l'accusé est en route");
        assert!(t.recus.contains_key(&id), "il peut encore revenir");
        t.confirmer(5, 8);
        assert_eq!(t.evenements.len(), 0);
        assert_eq!(t.recus.len(), 0);
    }

    /// Revue de la phase 2, constat 16 : sous `Panne::Doublon`, le réseau livre deux
    /// fois le même geste, la copie juste après l'original.
    #[test]
    fn le_reseau_double_un_geste() {
        let mut t = telephone(5);
        t.doubler = Oui::Oui;
        t.emettre(vec![1], (0, 0), Oui::Non, 0);
        let id = t.evenements[0].id;
        let copies: Vec<u64> = t
            .reseau
            .iter()
            .filter(|r| r.paquet == Paquet::Geste { id })
            .map(|r| r.livre_a)
            .collect();
        assert_eq!(copies.len(), 2, "{:?}", t.reseau);
        assert!(copies[1] > copies[0] && copies[1] <= copies[0] + 3);
    }

    /// Une coupure perd un accusé encore en route : le geste repart.
    #[test]
    fn un_accuse_perdu_fait_renvoyer_le_geste() {
        let mut t = telephone(4);
        t.emettre(vec![1], (0, 0), Oui::Non, 0);
        t.evenements[0].etat = EtatEvenement::Place { pas: 4, accuse: 8 };
        t.perdre_le_reseau(6);
        assert_eq!(t.evenements[0].etat, EtatEvenement::AEnvoyer);
        t.evenements[0].etat = EtatEvenement::Place { pas: 4, accuse: 8 };
        t.perdre_le_reseau(9);
        assert!(matches!(t.evenements[0].etat, EtatEvenement::Place { .. }));
    }
}
