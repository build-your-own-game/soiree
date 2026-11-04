//! Les lectures de la définitive (§ 4.4, § 4.6) : vues, état, demandes, ressources.
//!
//! Une lecture ne change pas l'état (D4). Si le jeu faute pendant une lecture,
//! l'instance n'est plus sûre : le cœur la reconstruit comme après une écriture,
//! garde la dernière vue valide de chaque regard, et rapporte la faute. Un retour
//! bien lu mais invalide (une vue mal formée) est rapporté sans reconstruire : la
//! lecture n'a pas échoué.

use alloc::string::String;
use alloc::vec::Vec;

use jeu_abi::{
    Appareil, Appel, Cle, IdDemande, MontreFil, Oui, Pas, RegardFil, Retour, SceneFil, SorteAsset,
    VueFil, decoder,
};

use crate::arrivee::DemandeArrivee;
use crate::coeur::DemandeEnCours;
use crate::instance::{Echec, Fabrique, Faute, Instance, PanneOutil};
use crate::journal::Ligne;
use crate::partie::{AttenteVisible, EtatVisible, LieuFaute, Lu, Partie, Vue, appeler, illisible};
use crate::regles;

impl<F: Fabrique> Partie<F> {
    /// Une lecture sur la définitive. Une faute du jeu est traitée ici : la
    /// définitive est reconstruite et la faute rapportée.
    pub(crate) fn lire(&mut self, appel: &Appel<'_>, nom: &'static str) -> Result<Vec<u8>, Lu> {
        match appeler(&mut self.definitive, appel, self.ouverture.budgets.lecture) {
            Ok(r) => Ok(r),
            Err(Echec::Faute(f)) => match self.faute_en_lecture(f.clone(), nom) {
                Ok(()) => Err(Lu::Faute(f)),
                Err(p) => Err(Lu::Panne(p)),
            },
            Err(Echec::Panne(p)) => Err(Lu::Panne(p)),
        }
    }

    /// Le jeu a fauté pendant une lecture : reconstruire, puis rapporter.
    pub(crate) fn faute_en_lecture(
        &mut self,
        f: Faute,
        nom: &'static str,
    ) -> Result<(), PanneOutil> {
        let carburant = self.definitive.carburant();
        self.reconstruire()?;
        let r = self.rapport(f, LieuFaute::Lecture { appel: nom }, carburant);
        self.dire(|a| a.fautes.push(r));
        Ok(())
    }

    /// Une lecture a rendu un retour invalide : rapporter, sans reconstruire.
    pub(crate) fn rapporter_lecture(&mut self, f: Faute, nom: &'static str) {
        let carburant = self.definitive.carburant();
        let r = self.rapport(f, LieuFaute::Lecture { appel: nom }, carburant);
        self.dire(|a| a.fautes.push(r));
    }

    /// Lit l'état après un pas qui a accepté quelque chose : le réveil, et les
    /// demandes à lancer ou à annuler (§ 2.10, D9).
    pub(crate) fn lire_etat(&mut self) -> Result<(), PanneOutil> {
        let connues: Vec<u64> = self.coeur.demandes.iter().map(|d| d.id).collect();
        let appel = Appel::Etat {
            connues: connues.clone(),
        };
        let r = match self.lire(&appel, "Etat") {
            Ok(r) => r,
            Err(Lu::Faute(_)) => return Ok(()),
            Err(Lu::Panne(p)) => return Err(p),
        };
        let e = match decoder::<Retour<'_>>(&r) {
            Ok(Retour::Etat(e)) => e,
            Ok(Retour::Illisible(x)) => return Err(illisible(x)),
            Ok(_) | Err(_) => {
                self.rapporter_lecture(Faute::SortieInvalide, "Etat");
                return Ok(());
            }
        };
        self.coeur.reveil = e.reveil;
        let mut lancer = Vec::new();
        let mut hors_plage = false;
        for id in regles::a_lancer(&connues, &e.demandes) {
            let Some(exposee) = self.coeur.exposer(id) else {
                // Une identité qui ne tient pas sur 32 bits ne vient pas de
                // `Sorties::id()` : la demande ne part pas, et la faute se rapporte.
                hors_plage = true;
                continue;
            };
            if self.coeur.deja_lancee(exposee) {
                // Listée de nouveau après une annulation : elle ne repart pas (CAT-76).
                self.coeur.demandes.push(DemandeEnCours {
                    id,
                    repondue: Oui::Oui,
                });
                continue;
            }
            // Le jeu liste une demande sans son contenu : rien à lancer.
            if let Some((_, d)) = e.nouvelles.iter().find(|(n, _)| *n == id) {
                self.coeur.lancer(exposee);
                self.coeur.demandes.push(DemandeEnCours {
                    id,
                    repondue: Oui::Non,
                });
                lancer.push((IdDemande::du_fil(exposee), DemandeArrivee::depuis_fil(d)));
            }
        }
        let mut annuler = Vec::new();
        for id in regles::a_annuler(&connues, &e.demandes) {
            if self.coeur.attend_reponse(id)
                && let Some(exposee) = self.coeur.exposer(id)
            {
                annuler.push(IdDemande::du_fil(exposee));
            }
            self.coeur.demandes.retain(|d| d.id != id);
        }
        self.dire(|a| {
            a.lancer.extend(lancer);
            a.annuler.extend(annuler);
        });
        if hors_plage {
            self.rapporter_lecture(Faute::SortieInvalide, "Etat");
        }
        Ok(())
    }

    /// Les vues de ces regards, validées (§ 4.4, § 6.1). Les téléphones voient la
    /// définitive. L'écran voit la tête quand elle est en avance : sa scène, dont
    /// chaque sous-arbre `definitif` vient de la définitive, avec les effets
    /// `Provisoire` de la tête et les `Definitive` de la définitive
    /// ([`regles::fusionner`]). Une vue invalide, ou une lecture fautive, laisse la
    /// dernière vue valide de son regard ; un regard qui n'en a jamais eu n'en reçoit
    /// pas.
    pub fn regarder(&mut self, regards: &[RegardFil]) -> Result<Vec<Vue>, PanneOutil> {
        let appel = Appel::Regarder {
            regards: regards.to_vec(),
        };
        let r = match self.lire(&appel, "Regarder") {
            Ok(r) => r,
            Err(Lu::Faute(_)) => return Ok(self.vues_gardees(regards)),
            Err(Lu::Panne(p)) => return Err(p),
        };
        let vues = match decoder::<Retour<'_>>(&r) {
            Ok(Retour::Vues(v)) if v.len() == regards.len() => v,
            Ok(Retour::Illisible(e)) => return Err(illisible(e)),
            Ok(_) | Err(_) => {
                let f = Faute::VueInvalide {
                    detail: String::from("le retour n'est pas une vue par regard"),
                };
                self.rapporter_lecture(f, "Regarder");
                return Ok(self.vues_gardees(regards));
            }
        };
        let mut rendu = Vec::new();
        for (regard, v) in regards.iter().zip(&vues) {
            match self.valider_vue(*regard, v) {
                Ok(scene) => {
                    let (octets, pas) = match scene {
                        Some(definitive) => self.scene_montree(definitive, v.octets)?,
                        None => (v.octets.to_vec(), self.coeur.pas),
                    };
                    if self.coeur.derniere_vue(*regard) != pas {
                        self.noter(Ligne::Vue {
                            regard: *regard,
                            pas,
                            apres: self.coeur.pas,
                        })?;
                        self.coeur.voir(*regard, pas);
                    }
                    let vue = Vue {
                        regard: *regard,
                        pas,
                        octets,
                    };
                    self.dernieres_vues.retain(|d| d.regard != *regard);
                    self.dernieres_vues.push(vue.clone());
                    rendu.push(vue);
                }
                Err(detail) => {
                    self.rapporter_lecture(Faute::VueInvalide { detail }, "Regarder");
                    rendu.extend(self.vues_gardees(core::slice::from_ref(regard)));
                }
            }
        }
        Ok(rendu)
    }

    /// La scène que l'écran montre, encodée, et son pas : celle de la définitive, ou,
    /// si la tête est en avance et que sa scène est valide, leur fusion par clé. Elle
    /// devient la scène que les entrées de l'écran doivent viser (SAL-152).
    fn scene_montree(
        &mut self,
        definitive: SceneFil,
        octets: &[u8],
    ) -> Result<(Vec<u8>, Pas), PanneOutil> {
        let mut tete = self.scene_de_la_tete()?;
        // Une scène de la tête invalide n'est pas montrée : la définitive la jugera à
        // son tour.
        if let Some((s, _)) = &tete
            && self.valider_scene(s).is_err()
        {
            self.tete_a_rejouer();
            tete = None;
        }
        if let Some((t, pas)) = tete {
            let fusion = regles::fusionner(&t, &definitive);
            let o = crate::partie::encoder_vers_vec(&fusion)?;
            self.derniere_scene = Some(fusion);
            return Ok((o, pas));
        }
        self.derniere_scene = Some(definitive);
        Ok((octets.to_vec(), self.coeur.pas))
    }

    /// Une vue se lit-elle, et tient-elle dans les limites déclarées (§ 6.1) ? Rend
    /// la scène, pour l'écran.
    fn valider_vue(
        &mut self,
        regard: RegardFil,
        v: &VueFil<'_>,
    ) -> Result<Option<SceneFil>, String> {
        if v.regard != regard {
            return Err(alloc::format!(
                "vue de {:?} pour le regard {regard:?}",
                v.regard
            ));
        }
        let limites = self.declaration.limites;
        if v.octets.len() > usize::try_from(limites.vue).unwrap_or(usize::MAX) {
            return Err(alloc::format!(
                "{} octets, au-delà de {}",
                v.octets.len(),
                limites.vue
            ));
        }
        let max_effets = usize::from(limites.effets);
        match regard {
            RegardFil::Ecran => {
                let scene =
                    decoder::<SceneFil>(v.octets).map_err(|e| alloc::format!("scène : {e}"))?;
                if scene.effets.len() > max_effets {
                    return Err(alloc::format!(
                        "{} effets, au-delà de {max_effets}",
                        scene.effets.len()
                    ));
                }
                self.valider_scene(&scene)
                    .map_err(|e| alloc::format!("scène : {e}"))?;
                Ok(Some(scene))
            }
            RegardFil::Appareil(_) => {
                let montre =
                    decoder::<MontreFil<'_>>(v.octets).map_err(|e| alloc::format!("vue : {e}"))?;
                if montre.effets.len() > max_effets {
                    return Err(alloc::format!(
                        "{} effets, au-delà de {max_effets}",
                        montre.effets.len()
                    ));
                }
                Ok(None)
            }
        }
    }

    /// La scène est-elle valide (§ 6.1) ? Les images que ni les assets ni le cache ne
    /// connaissent sont demandées au jeu, une fois chacune (`Ressources`).
    fn valider_scene(&mut self, scene: &SceneFil) -> Result<(), regles::DefautScene> {
        let mut polices = Vec::new();
        let mut images = Vec::new();
        for a in &self.declaration.assets {
            match a.sorte {
                SorteAsset::Police => polices.push(a.cle),
                SorteAsset::Image => images.push(a.cle),
                SorteAsset::Son => {}
            }
        }
        let inconnues: Vec<u64> = regles::images_citees(scene)
            .into_iter()
            .filter(|c| !images.contains(c))
            .filter(|c| !self.images.iter().any(|(k, _)| *k == c.fil()))
            .map(Cle::fil)
            .collect();
        if !inconnues.is_empty() {
            let trouvees: Vec<u64> = self
                .ressources(&inconnues)
                .map(|v| v.into_iter().map(|(c, _)| c).collect())
                .unwrap_or_default();
            for c in inconnues {
                let connue = if trouvees.contains(&c) {
                    Oui::Oui
                } else {
                    Oui::Non
                };
                self.images.push((c, connue));
            }
        }
        images.extend(
            self.images
                .iter()
                .filter(|(_, connue)| *connue == Oui::Oui)
                .map(|(c, _)| Cle::du_fil(*c)),
        );
        regles::valider_scene(scene, &self.declaration.ecran, &polices, &images)
    }

    fn vues_gardees(&self, regards: &[RegardFil]) -> Vec<Vue> {
        regards
            .iter()
            .filter_map(|r| self.dernieres_vues.iter().find(|v| v.regard == *r).cloned())
            .collect()
    }

    /// Ce que la définitive dit d'elle-même : phase, attente, accueil, cas.
    pub fn etat(&mut self) -> Result<EtatVisible, Echec> {
        let connues: Vec<u64> = self.coeur.demandes.iter().map(|d| d.id).collect();
        let r = match self.lire(&Appel::Etat { connues }, "Etat") {
            Ok(r) => r,
            Err(Lu::Faute(f)) => return Err(Echec::Faute(f)),
            Err(Lu::Panne(p)) => return Err(Echec::Panne(p)),
        };
        match decoder::<Retour<'_>>(&r) {
            Ok(Retour::Etat(e)) => Ok(EtatVisible {
                phase_indice: e.phase_indice,
                terminale: e.terminale,
                attente: AttenteVisible {
                    echeance: e.attente.echeance.map(Pas::du_fil),
                    libre: e.attente.libre.map(String::from),
                    joueurs: e
                        .attente
                        .joueurs
                        .iter()
                        .map(|(a, g)| {
                            (Appareil::du_fil(*a), g.iter().map(|o| o.to_vec()).collect())
                        })
                        .collect(),
                    ecran: e.attente.ecran.iter().map(|o| o.to_vec()).collect(),
                },
                accueil: e.accueil,
                cas_indice: e.cas_indice,
            }),
            Ok(Retour::Illisible(x)) => Err(Echec::Panne(illisible(x))),
            Ok(_) | Err(_) => {
                self.rapporter_lecture(Faute::SortieInvalide, "Etat");
                Err(Echec::Faute(Faute::SortieInvalide))
            }
        }
    }

    /// La forme canonique de la définitive, secrets compris, ses morceaux recollés
    /// (`Appel::Canonique`, une lecture sous le budget `canonique`).
    ///
    /// Le cœur ne s'en sert pas pour jouer. Elle est la seule mesure de l'état
    /// commune au module et à la copie native, dont les empreintes d'instance ne
    /// mesurent pas la même chose : l'épreuve compare ainsi les deux (E9). Une faute
    /// du jeu se traite comme celle de toute lecture : la définitive est reconstruite,
    /// et la faute rapportée.
    pub fn canonique(&mut self) -> Result<Vec<u8>, Echec> {
        let mut tout = Vec::new();
        let mut morceau = 0;
        loop {
            let appel = Appel::Canonique { morceau };
            let r = match appeler(
                &mut self.definitive,
                &appel,
                self.ouverture.budgets.canonique,
            ) {
                Ok(r) => r,
                Err(Echec::Faute(f)) => {
                    self.faute_en_lecture(f.clone(), "Canonique")?;
                    return Err(Echec::Faute(f));
                }
                Err(Echec::Panne(p)) => return Err(Echec::Panne(p)),
            };
            match decoder::<Retour<'_>>(&r) {
                Ok(Retour::Morceau { octets, suivant }) => {
                    tout.extend_from_slice(octets);
                    match suivant {
                        // Un numéro qui ne monte pas ferait tourner la lecture sans fin.
                        Some(s) if s > morceau => morceau = s,
                        Some(_) => {
                            self.rapporter_lecture(Faute::SortieInvalide, "Canonique");
                            return Err(Echec::Faute(Faute::SortieInvalide));
                        }
                        None => return Ok(tout),
                    }
                }
                Ok(Retour::Illisible(x)) => return Err(Echec::Panne(illisible(x))),
                Ok(_) | Err(_) => {
                    self.rapporter_lecture(Faute::SortieInvalide, "Canonique");
                    return Err(Echec::Faute(Faute::SortieInvalide));
                }
            }
        }
    }

    /// Les octets des ressources citées, que le jeu connaît.
    pub fn ressources(&mut self, cles: &[u64]) -> Result<Vec<(u64, Vec<u8>)>, PanneOutil> {
        let appel = Appel::Ressources {
            cles: cles.to_vec(),
        };
        let r = match self.lire(&appel, "Ressources") {
            Ok(r) => r,
            Err(Lu::Faute(_)) => return Ok(Vec::new()),
            Err(Lu::Panne(p)) => return Err(p),
        };
        match decoder::<Retour<'_>>(&r) {
            Ok(Retour::Ressources(v)) => Ok(v.into_iter().map(|(c, o)| (c, o.to_vec())).collect()),
            Ok(Retour::Illisible(x)) => Err(illisible(x)),
            Ok(_) | Err(_) => {
                self.rapporter_lecture(Faute::SortieInvalide, "Ressources");
                Ok(Vec::new())
            }
        }
    }
}
