//! Ce qui pilote une instance de `devine` dans les tests : encoder un appel, décoder le
//! retour, compter l'ordre des entrées. Le même pilote sert à la copie native et au
//! module.

#![expect(
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::unwrap_used,
    clippy::wildcard_enum_match_arm,
    reason = "code de test : un échec s'y lit comme l'échec du test qui l'appelle"
)]
#![expect(
    dead_code,
    reason = "chaque fichier de test n'emploie qu'une part du pilote"
)]

use devine::{ActionEcran, Curseur, Devine, GesteTel, Prenom};
use jeu::prelude::*;
use jeu_abi::{
    Appel, EcranFil, EntreeFil, IssueFil, QuandFil, RegardFil, Retour, SorteAppel, decoder, encoder,
};

/// Ce qui exécute un appel encodé.
pub trait Porte {
    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8]) -> Result<Vec<u8>, String>;
}

impl Porte for jeu_natif::InstanceNative<Devine> {
    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8]) -> Result<Vec<u8>, String> {
        partie::Instance::appeler(self, sorte, appel, u64::MAX).map_err(|e| e.to_string())
    }
}

/// Le pilote.
pub struct Pilote<P: Porte> {
    pub porte: P,
    ordre: u64,
    pub id_geste: u64,
}

pub const A: u64 = 0x1111_2222_3333_4444;
pub const B: u64 = 0x5555_6666_7777_8888;

pub fn quand(fait: u64, vu: u64, id: u64) -> QuandFil {
    QuandFil {
        fait,
        fraction: 0,
        recu: fait,
        vu,
        id,
    }
}

/// Ce que le jeu a décidé d'une entrée, sans ses octets.
#[derive(Debug, Clone, PartialEq)]
pub enum Decide {
    Acceptee,
    Refusee(devine::Avis),
    Ignoree,
    Attente,
}

impl<P: Porte> Pilote<P> {
    pub fn new(porte: P) -> Self {
        Pilote {
            porte,
            ordre: 0,
            id_geste: 0,
        }
    }

    /// Les octets du retour, tels quels.
    pub fn brut(&mut self, a: &Appel<'_>) -> Result<Vec<u8>, String> {
        let mut o = Vec::new();
        encoder(a, &mut o).unwrap();
        self.porte.appeler(a.sorte(), &o)
    }

    /// Le retour, décodé et rendu à `f`.
    pub fn avec<R>(&mut self, a: &Appel<'_>, f: impl FnOnce(Retour<'_>) -> R) -> R {
        let o = self.brut(a).unwrap();
        f(decoder::<Retour<'_>>(&o).unwrap())
    }

    pub fn creer(&mut self, graine: [u8; 32], garde: Option<&[u8]>) {
        let mut presents = Vec::new();
        encoder(&Vec::<Presence>::new(), &mut presents).unwrap();
        self.avec(
            &Appel::Creer {
                graine,
                pas: 0,
                heure: 1_700_000_000_000,
                salle: TexteCourt::new("SALLE"),
                presents: &presents,
                garde,
            },
            |r| {
                assert!(
                    matches!(r, Retour::Issue(IssueFil::Acceptee { .. })),
                    "{r:?}"
                );
            },
        );
    }

    /// Une entrée, placée à `place`.
    pub fn entree(&mut self, place: u64, e: &EntreeFil<'_>) -> Decide {
        let mut o = Vec::new();
        encoder(e, &mut o).unwrap();
        self.ordre += 1;
        let ordre = self.ordre;
        self.avec(
            &Appel::Entree {
                ordre,
                place,
                entree: &o,
            },
            |r| match r {
                Retour::Issue(IssueFil::Acceptee { .. }) => Decide::Acceptee,
                Retour::Issue(IssueFil::Refusee { avis }) => {
                    Decide::Refusee(decoder(avis).unwrap())
                }
                Retour::Issue(IssueFil::Ignoree) => Decide::Ignoree,
                Retour::Issue(IssueFil::Attente) => Decide::Attente,
                autre => panic!("retour inattendu : {autre:?}"),
            },
        )
    }

    pub fn geste(&mut self, de: u64, g: &GesteTel, place: u64, vu: u64) -> Decide {
        let mut o = Vec::new();
        encoder(g, &mut o).unwrap();
        self.id_geste += 1;
        let q = quand(place, vu, self.id_geste);
        self.entree(
            place,
            &EntreeFil::Geste {
                de,
                octets: &o,
                quand: q,
            },
        )
    }

    pub fn rejoindre(&mut self, de: u64, nom: &str, place: u64) -> Decide {
        self.geste(
            de,
            &GesteTel::Rejoindre(Texte::<Prenom>::nettoyer(nom)),
            place,
            place,
        )
    }

    pub fn curseur(&mut self, de: u64, c: i64, place: u64) -> Decide {
        let mut o = Vec::new();
        encoder(&Curseur(Borne::new(c).unwrap()), &mut o).unwrap();
        self.id_geste += 1;
        let q = quand(place, place, self.id_geste);
        self.entree(
            place,
            &EntreeFil::Tenu {
                de,
                octets: &o,
                cause: CauseTenu::Geste,
                quand: q,
            },
        )
    }

    pub fn lancer(&mut self, place: u64, vu: u64) -> Decide {
        let mut o = Vec::new();
        encoder(&ActionEcran::Lancer, &mut o).unwrap();
        self.id_geste += 1;
        let q = quand(place, vu, self.id_geste);
        self.entree(
            place,
            &EntreeFil::Ecran {
                quoi: EcranFil::Action(&o),
                quand: q,
            },
        )
    }

    pub fn temps(&mut self, place: u64) -> Decide {
        self.entree(place, &EntreeFil::Temps { saut: 0, heure: 0 })
    }

    /// La forme canonique entière, morceau par morceau.
    pub fn canonique(&mut self) -> Vec<u8> {
        self.morceaux(|m| Appel::Canonique { morceau: m })
    }

    /// La projection pour un regard, entière.
    pub fn projection(&mut self, regard: RegardFil) -> Vec<u8> {
        self.morceaux(|m| Appel::Projeter { regard, morceau: m })
    }

    fn morceaux(&mut self, appel: impl Fn(u32) -> Appel<'static>) -> Vec<u8> {
        let mut tout = Vec::new();
        let mut m = 0;
        loop {
            let suivant = self.avec(&appel(m), |r| match r {
                Retour::Morceau { octets, suivant } => {
                    tout.extend_from_slice(octets);
                    suivant
                }
                autre => panic!("retour inattendu : {autre:?}"),
            });
            match suivant {
                Some(s) => m = s,
                None => return tout,
            }
        }
    }

    /// Les octets de la vue de chaque regard.
    pub fn vues(&mut self, regards: &[RegardFil]) -> Vec<Vec<u8>> {
        self.avec(
            &Appel::Regarder {
                regards: regards.to_vec(),
            },
            |r| match r {
                Retour::Vues(v) => v.iter().map(|v| v.octets.to_vec()).collect(),
                autre => panic!("retour inattendu : {autre:?}"),
            },
        )
    }

    /// La vue d'un téléphone, décodée.
    pub fn vue_telephone(&mut self, de: u64) -> devine::VueTel {
        let o = self.vues(&[RegardFil::Appareil(de)]).remove(0);
        let m: jeu_abi::MontreFil<'_> = decoder(&o).unwrap();
        decoder(m.vue).unwrap()
    }

    /// Les textes de la scène de l'écran.
    pub fn textes_ecran(&mut self) -> Vec<String> {
        let o = self.vues(&[RegardFil::Ecran]).remove(0);
        let s: jeu_abi::SceneFil = decoder(&o).unwrap();
        s.noeuds
            .iter()
            .filter_map(|n| match &n.sorte {
                jeu_abi::SorteNoeud::Texte { texte, .. } => Some(texte.clone()),
                _ => None,
            })
            .collect()
    }
}
