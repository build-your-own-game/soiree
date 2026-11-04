// Un jeu minimal pour les tests de compilation : chaque cas choisit ses types et sa
// déclaration, puis `jeu::module!` vérifie l'accord en `const` (§ 2.12, D28).
macro_rules! jeu_minimal {
    (
        telephones: $telephones:expr,
        geste: $geste:ty,
        tenu: $tenu:ty,
        vue: $vue:ty,
        secrets: $secrets:expr,
        prediction: $prediction:expr,
        champ: $champ:ty $(,)?
    ) => {
        use jeu::prelude::*;

        #[jeu::etat]
        pub struct Minimal {
            pub valeur: u8,
            pub champ: $champ,
        }

        #[jeu::message]
        #[derive(Copy, Eq, Variantes)]
        pub enum Etape {
            Seule,
        }

        impl Phase for Etape {
            fn terminale(self) -> Oui {
                Oui::Non
            }
        }

        const TENU: ReglesTenu = ReglesTenu {
            fraicheur: Applicable::SansObjet(jeu::raison!("rien ne se relâche ici")),
            age_max: Applicable::SansObjet(jeu::raison!("rien ne vieillit ici")),
            debit_max: 10,
        };

        impl Jeu for Minimal {
            type ActionEcran = Aucun;
            type GesteTelephone = $geste;
            type TenuTelephone = $tenu;
            type VueTelephone = $vue;
            type Avis = Aucun;
            type Phase = Etape;
            type Garde = Aucun;
            type Precedent = Aucun;

            const DECLARATION: Declaration = Declaration {
                nom: "minimal",
                places: Places::joueurs(1, 2),
                rattrapage: 1,
                fenetre: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
                antidatage_max: Duree::ms(30),
                pause: Pause::Aucune(jeu::raison!("un jeu de test minimal")),
                ecran: FormeEcran::Page { largeur_max: 800 },
                telephones: $telephones,
                manettes: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
                prediction: $prediction,
                hasard: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
                secrets: $secrets,
                fin: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
                capacites: Voulues {
                    vibration: Oui::Non,
                    mouvement: Oui::Non,
                    son: Oui::Non,
                    ecran_allume: Oui::Non,
                    micro: Oui::Non,
                    pad_natif: Oui::Non,
                },
                limites: Limites { geste: 64, file: 4, vue: 1024, effets: 2, memoire: 32, brouillon: 65536 },
                diagnostic: CodeTouche::new("KeyD"),
                assets: &[],
                mecaniques: &[],
            };

            fn creer(_: Creation<'_, Self>) -> Self {
                todo!()
            }
            fn appliquer(avant: Avant<'_, Self>, _: Entree<'_, Self>) -> Issue<Self> {
                avant.ignorer()
            }
            fn vue_ecran(&self, _: &Regard, _: &mut Scene<Aucun>) {}
            fn vue_telephone(&self, _: &Regard) -> Montre<$vue> {
                todo!()
            }
            fn ressource(&self, _: Cle) -> Option<&[u8]> {
                None
            }
            fn phase(&self) -> Etape {
                Etape::Seule
            }
            fn reveil(&self) -> Reveil {
                Reveil::Jamais
            }
            fn attente(&self, _: &mut Attente<'_, Self>) {}
            fn accueil(&self) -> Accueil {
                Accueil::Ouverte
            }
            fn demandes<'s>(&'s self, _: &mut Demandes<'s>) {}
            fn previsible(_: &$geste) -> Prevision {
                Prevision::Non
            }
            fn garder(&self) -> Aucun {
                todo!()
            }
            fn convertir(a: Aucun) -> Result<Self, Raison> {
                match a {}
            }
        }

        impl Epreuve for Minimal {
            type Resume = u8;
            type Cas = Etape;
            fn resume(&self, _: &Autorite) -> u8 {
                self.valeur
            }
            fn invariants(&self, _: &Autorite, _: &mut Verif) {}
            fn transition(_: &u8, _: &u8, _: &mut Verif) {}
            fn bot_telephone(_: &Self, _: &Montre<$vue>, b: &mut Bot<'_, Self>) {
                b.rien();
            }
            fn bot_ecran(_: &Self, b: &mut Bot<'_, Self>) {
                b.rien();
            }
            fn cas(&self) -> Etape {
                Etape::Seule
            }
            fn pire_cas(_: Etape, _: &Construction<'_>) -> Self {
                todo!()
            }
            fn proprietes() -> Proprietes<Self> {
                Proprietes {
                    coupure_transparente: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
                    permutation: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
                }
            }
        }

        jeu::module!(Minimal);
    };
}
