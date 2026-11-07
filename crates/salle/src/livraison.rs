//! Le module du jeu et les ressources de la page, de l'autorité aux téléphones (PHASE-3,
//! § 2.5, § 2.10, P18 ; lot 12).
//!
//! **Sans entrées-sorties.** L'autorité découpe le module livré en morceaux
//! ([`MORCEAU_MODULE`]) et l'envoie **par fenêtre** : au plus [`FENETRE_MODULE`] morceaux
//! non accusés par téléphone (`VersAutorite::ModuleRecu`), et aucun tant que le relais dit
//! la file de ce téléphone encombrée (`DuRelais::Encombre`), jusqu'au prochain accusé. Sa
//! WebSocket vers le relais, qui porte le temps réel de tous, n'est ainsi jamais pleine
//! d'un module : un téléphone qui rejoint en pleine partie ne retarde la vue de personne.
//!
//! Les ressources de la page (polices, images : ce que `Ressources` rend pour chaque asset
//! déclaré) suivent le module, dans la même fenêtre, une `VersAppareil::Ressource` chacune.
//! Elles n'ont pas de numéro sur le fil : la `i`-ième s'accuse par
//! `ModuleRecu { morceau: total + i }`, où `total` est le nombre de morceaux du module. La
//! forme des messages ne change donc pas (`salle::fil::PROTOCOLE` reste 1).
//!
//! Le téléphone ([`Reception`]) rassemble les morceaux, vérifie le BLAKE3 du module
//! entier contre l'empreinte annoncée, et le dit connu dans son bonjour suivant
//! (`module_connu`) : un téléphone qui revient sans avoir rechargé ne le reçoit pas deux
//! fois. **Un écart à PHASE-3 (§ 2.8, point 4)** : c'est le module **livré** qui part, et
//! non le module réécrit ; le téléphone le réécrit lui-même sous `PHASE_3`, comme le
//! Worker, avant de le compiler (une seule définition de la réécriture, dans
//! `moteur-navigateur::charger`), et l'empreinte qu'il vérifie est celle du catalogue.

/// Choisi : la taille d'un morceau du module, sous `liaison::TRAME_MAX` (256 Kio) avec
/// la marge des en-têtes et du chiffrement. Un module de 4 Mio (`TAILLE_MODULE_MAX`) en
/// fait 64.
pub const MORCEAU_MODULE: usize = 64 * 1024;

/// PHASE-3, § 2.5 : au plus deux morceaux non accusés par téléphone.
pub const FENETRE_MODULE: u32 = 2;

/// Le plus de morceaux qu'un téléphone accepte d'un module : 4 Mio, plus un, pour ce que
/// l'autorité ne refuserait pas elle-même (choisi : la borne du chargement,
/// `TAILLE_MODULE_MAX`, est vérifiée en amont).
pub const MORCEAUX_MAX: u32 = 65;

/// Les ressources qu'un téléphone garde au plus (choisi : au-dessus des assets d'un jeu,
/// que `jeu_abi::Limites` borne déjà).
pub const RESSOURCES_MAX: usize = 1024;

/// Ce que l'autorité livre aux téléphones : le module en morceaux, puis les ressources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Livraison {
    /// Le BLAKE3 du module livré.
    pub empreinte: [u8; 32],
    /// Les morceaux du module, dans l'ordre.
    pub morceaux: Vec<Vec<u8>>,
    /// Les ressources de la page, par clé d'asset.
    pub ressources: Vec<(u64, Vec<u8>)>,
}

impl Livraison {
    /// Le module livré, découpé ; les ressources s'ajoutent quand la partie les a dites.
    pub fn new(empreinte: [u8; 32], module: &[u8]) -> Livraison {
        Livraison {
            empreinte,
            morceaux: module.chunks(MORCEAU_MODULE).map(<[u8]>::to_vec).collect(),
            ressources: Vec::new(),
        }
    }

    /// Le nombre de morceaux du module.
    pub fn total(&self) -> u32 {
        u32::try_from(self.morceaux.len()).unwrap_or(u32::MAX)
    }

    /// Le nombre de messages à accuser : les morceaux, puis les ressources.
    pub fn messages(&self) -> u32 {
        self.total()
            .saturating_add(u32::try_from(self.ressources.len()).unwrap_or(u32::MAX))
    }
}

/// Ce qui part de la livraison vers un téléphone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AEnvoyer {
    /// Le morceau `n` du module.
    Morceau(u32),
    /// La ressource `i`.
    Ressource(usize),
}

/// Où en est la livraison vers un téléphone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnvoiEnCours {
    /// Le prochain message à envoyer.
    pub prochain: u32,
    /// Les messages accusés, dans l'ordre (la file fiable les garde dans l'ordre).
    pub accuses: u32,
    /// Le relais a dit la file de ce téléphone encombrée : rien ne part avant le prochain
    /// accusé.
    pub encombre: bool,
}

impl EnvoiEnCours {
    /// Les messages qui peuvent partir maintenant, dans la fenêtre.
    pub fn a_envoyer(&mut self, l: &Livraison) -> Vec<AEnvoyer> {
        let mut sortie = Vec::new();
        // Encombré : rien ne part avant le prochain accusé ; s'il n'y a rien en vol (aucun
        // accusé à attendre), un seul message à la fois.
        if self.encombre && self.prochain > self.accuses {
            return sortie;
        }
        let fenetre = if self.encombre { 1 } else { FENETRE_MODULE };
        let total = l.total();
        while self.prochain < l.messages() && self.prochain.saturating_sub(self.accuses) < fenetre {
            let n = self.prochain;
            sortie.push(if n < total {
                AEnvoyer::Morceau(n)
            } else {
                AEnvoyer::Ressource(usize::try_from(n.saturating_sub(total)).unwrap_or(0))
            });
            self.prochain = n.saturating_add(1);
        }
        sortie
    }

    /// Le téléphone accuse le message `n`.
    pub fn accuser(&mut self, n: u32) {
        if n < self.prochain {
            self.accuses = self.accuses.max(n.saturating_add(1));
            self.encombre = false;
        }
    }

    /// Tout est-il livré et accusé ?
    pub fn fini(&self, l: &Livraison) -> bool {
        self.accuses >= l.messages()
    }
}

/// Le module que reçoit un téléphone, et les ressources qui le suivent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reception {
    empreinte: Option<[u8; 32]>,
    morceaux: Vec<Option<Vec<u8>>>,
    complet: Option<Vec<u8>>,
    ressources: Vec<(u64, Vec<u8>)>,
    /// Les modules rassemblés dont le BLAKE3 n'était pas l'empreinte annoncée.
    pub faux: u64,
}

impl Reception {
    /// L'empreinte du module que la salle joue, dite par l'accueil : un autre module
    /// commencé est oublié.
    pub fn annoncer(&mut self, empreinte: [u8; 32]) {
        if self.empreinte != Some(empreinte) {
            *self = Reception {
                empreinte: Some(empreinte),
                faux: self.faux,
                ..Reception::default()
            };
        }
    }

    /// Un morceau du module. Rend le numéro à accuser, ou rien si le morceau ne se
    /// range pas (un autre module, hors bornes).
    pub fn morceau(
        &mut self,
        empreinte: [u8; 32],
        morceau: u32,
        total: u32,
        octets: &[u8],
    ) -> Option<u32> {
        if total == 0 || total > MORCEAUX_MAX || morceau >= total {
            return None;
        }
        if self.empreinte != Some(empreinte) {
            self.annoncer(empreinte);
        }
        let n = usize::try_from(total).ok()?;
        if self.morceaux.len() != n {
            self.morceaux = vec![None; n];
            self.complet = None;
            self.ressources.clear();
        }
        let i = usize::try_from(morceau).ok()?;
        if let Some(place) = self.morceaux.get_mut(i) {
            *place = Some(octets.to_vec());
        }
        if self.complet.is_none() && self.morceaux.iter().all(Option::is_some) {
            let entier: Vec<u8> = self.morceaux.iter().flatten().flatten().copied().collect();
            if blake3::hash(&entier).as_bytes() == &empreinte {
                self.complet = Some(entier);
            } else {
                self.faux = self.faux.saturating_add(1);
                self.morceaux = vec![None; n];
            }
        }
        Some(morceau)
    }

    /// Une ressource ; rend le numéro à accuser (`total + i`).
    pub fn ressource(&mut self, cle: u64, octets: &[u8]) -> Option<([u8; 32], u32)> {
        let empreinte = self.empreinte?;
        let total = u32::try_from(self.morceaux.len()).ok()?;
        if self.ressources.len() >= RESSOURCES_MAX {
            return None;
        }
        let i = u32::try_from(self.ressources.len()).ok()?;
        match self.ressources.iter_mut().find(|(c, _)| *c == cle) {
            Some(r) => r.1 = octets.to_vec(),
            None => self.ressources.push((cle, octets.to_vec())),
        }
        Some((empreinte, total.saturating_add(i)))
    }

    /// L'empreinte annoncée, s'il y en a une.
    pub const fn empreinte(&self) -> Option<[u8; 32]> {
        self.empreinte
    }

    /// Le module entier, vérifié.
    pub fn module(&self) -> Option<([u8; 32], &[u8])> {
        Some((self.empreinte?, self.complet.as_deref()?))
    }

    /// Les ressources reçues.
    pub fn ressources(&self) -> &[(u64, Vec<u8>)] {
        &self.ressources
    }

    /// Les morceaux reçus, sur le total attendu.
    pub fn progres(&self) -> (usize, usize) {
        (
            self.morceaux.iter().filter(|m| m.is_some()).count(),
            self.morceaux.len(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_fenetre_ne_laisse_partir_que_deux_morceaux_non_accuses() {
        let module = vec![7u8; MORCEAU_MODULE * 3 + 5];
        let mut l = Livraison::new(*blake3::hash(&module).as_bytes(), &module);
        l.ressources.push((9, vec![1, 2, 3]));
        assert_eq!(l.total(), 4);
        let mut e = EnvoiEnCours::default();
        assert_eq!(
            e.a_envoyer(&l),
            [AEnvoyer::Morceau(0), AEnvoyer::Morceau(1)]
        );
        assert_eq!(e.a_envoyer(&l), []);
        e.accuser(0);
        assert_eq!(e.a_envoyer(&l), [AEnvoyer::Morceau(2)]);
        e.encombre = true;
        e.accuser(9); // jamais envoyé : ignoré, la file reste encombrée
        assert_eq!(e.a_envoyer(&l), []);
        // Encombré sans rien en vol : un seul message à la fois, sans quoi rien ne
        // repartirait jamais (aucun accusé à attendre).
        let mut f = EnvoiEnCours {
            prochain: 2,
            accuses: 2,
            encombre: true,
        };
        assert_eq!(f.a_envoyer(&l), [AEnvoyer::Morceau(2)]);
        assert_eq!(f.a_envoyer(&l), []);
        e.accuser(1);
        assert_eq!(e.a_envoyer(&l), [AEnvoyer::Morceau(3)]);
        e.accuser(2);
        e.accuser(3);
        assert_eq!(e.a_envoyer(&l), [AEnvoyer::Ressource(0)]);
        assert!(!e.fini(&l));
        e.accuser(4);
        assert!(e.fini(&l));
    }

    #[test]
    fn un_module_se_rassemble_et_se_verifie() {
        let module: Vec<u8> = (0..MORCEAU_MODULE * 2 + 17)
            .map(|i| u8::try_from(i % 251).unwrap_or(0))
            .collect();
        let empreinte = *blake3::hash(&module).as_bytes();
        let l = Livraison::new(empreinte, &module);
        let mut r = Reception::default();
        r.annoncer(empreinte);
        // Dans le désordre, et un morceau en double.
        for n in [2u32, 0, 0, 1] {
            let octets = l.morceaux.get(usize::try_from(n).unwrap_or(0)).cloned();
            assert_eq!(
                r.morceau(empreinte, n, l.total(), &octets.unwrap_or_default()),
                Some(n)
            );
        }
        assert_eq!(r.module().map(|(_, m)| m.len()), Some(module.len()));
        assert_eq!(r.ressource(5, &[1]), Some((empreinte, 3)));
        assert_eq!(r.ressource(6, &[2]), Some((empreinte, 4)));
        // Une autre empreinte annoncée oublie le module.
        r.annoncer([1; 32]);
        assert!(r.module().is_none());
    }

    #[test]
    fn un_module_qui_n_est_pas_celui_annonce_est_refuse() {
        let module = vec![3u8; 10];
        let mut r = Reception::default();
        assert_eq!(r.morceau([0; 32], 0, 1, &module), Some(0));
        assert!(r.module().is_none());
        assert_eq!(r.faux, 1);
        assert_eq!(r.morceau([0; 32], 1, 1, &module), None);
        assert_eq!(r.morceau([0; 32], 0, MORCEAUX_MAX + 1, &module), None);
    }
}
