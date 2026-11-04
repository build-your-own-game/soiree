//! Les collections du commun : [`Partage`] (§ 2.2) et [`Appareils`] (§ 2.5).

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::fmt;

use jeu_abi::{Appareil, Oui};

use crate::cache::{Ecrivain, Illisible, Liseur, forme};
use crate::forme::{Contient, Etat, Forme, Message};
use crate::secret::Regard;

/// Un contenu immuable partagé entre plusieurs copies de l'état, comme une image dans
/// l'historique de Tier list (TL-20). La forme canonique l'écrit une fois par
/// allocation.
///
/// Il ne donne que `&T` : rien ne le modifie après sa création. Ce n'est pas un
/// `Message`.
pub struct Partage<T: Etat>(Rc<T>);

impl<T: Etat> Partage<T> {
    /// Partage `v`.
    pub fn new(v: T) -> Self {
        Partage(Rc::new(v))
    }

    /// Le contenu.
    pub fn get(&self) -> &T {
        &self.0
    }

    /// Les deux partagent-ils le même contenu, la même allocation ? Sans lire le
    /// contenu : dire qu'une action n'a rien changé coûte une comparaison (TL-18,
    /// besoin 16 des jeux). La forme canonique écrit un partage une fois par
    /// allocation et le rend partagé à la relecture : la réponse survit à une reprise.
    pub fn meme(a: &Partage<T>, b: &Partage<T>) -> Oui {
        if Rc::ptr_eq(&a.0, &b.0) {
            Oui::Oui
        } else {
            Oui::Non
        }
    }
}

impl<T: Etat> Clone for Partage<T> {
    fn clone(&self) -> Self {
        Partage(Rc::clone(&self.0))
    }
}

impl<T: Etat> fmt::Debug for Partage<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Partage").field(self.get()).finish()
    }
}

impl<T: Etat> Forme for Partage<T> {
    const EMPREINTE: u64 = forme::de("Partage", T::EMPREINTE);
    const SECRET: Contient = T::SECRET;
    const VIDE: Contient = Contient::Non;
}

impl<T: Etat> Etat for Partage<T> {
    fn projeter(&self, r: &Regard) -> Self {
        // Sans secret dedans, la projection est le contenu lui-même : on garde le partage.
        match T::SECRET {
            Contient::Non => self.clone(),
            Contient::Oui => Partage::new(self.get().projeter(r)),
        }
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        if e.partage(&self.0) {
            self.get().ecrire(e);
        }
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        Ok(Partage(l.partage(|l| T::lire(l))?))
    }
}

/// Une table d'appareils, dans l'ordre où ils y sont entrés (D19). `Appareil` n'est ni
/// ordonné ni hachable : l'ordre d'arrivée est le seul ordre.
#[derive(Clone, PartialEq, Eq)]
pub struct Appareils<V> {
    lignes: Vec<(Appareil, V)>,
}

impl<V> Appareils<V> {
    /// Une table vide.
    pub const fn new() -> Self {
        Appareils { lignes: Vec::new() }
    }

    /// La valeur de cet appareil.
    pub fn get(&self, a: Appareil) -> Option<&V> {
        self.lignes.iter().find(|(b, _)| *b == a).map(|(_, v)| v)
    }

    /// La valeur de cet appareil, à modifier.
    pub fn get_mut(&mut self, a: Appareil) -> Option<&mut V> {
        self.lignes
            .iter_mut()
            .find(|(b, _)| *b == a)
            .map(|(_, v)| v)
    }

    /// Remplace sur place la valeur d'un appareil déjà là, et rend l'ancienne ; sinon
    /// l'ajoute à la fin.
    pub fn inserer(&mut self, a: Appareil, v: V) -> Option<V> {
        if let Some(ancienne) = self.get_mut(a) {
            return Some(core::mem::replace(ancienne, v));
        }
        self.lignes.push((a, v));
        None
    }

    /// Retire un appareil, et rend sa valeur. L'ordre des autres ne change pas.
    pub fn retirer(&mut self, a: Appareil) -> Option<V> {
        let i = self.lignes.iter().position(|(b, _)| *b == a)?;
        Some(self.lignes.remove(i).1)
    }

    /// Les appareils et leurs valeurs, dans l'ordre d'entrée.
    pub fn iter(&self) -> impl Iterator<Item = (Appareil, &V)> {
        self.lignes.iter().map(|(a, v)| (*a, v))
    }

    /// Les appareils et leurs valeurs, à modifier.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Appareil, &mut V)> {
        self.lignes.iter_mut().map(|(a, v)| (*a, v))
    }

    /// Le nombre d'appareils.
    pub fn len(&self) -> usize {
        self.lignes.len()
    }

    /// La table est-elle vide ?
    pub fn is_empty(&self) -> bool {
        self.lignes.is_empty()
    }
}

impl<V> Default for Appareils<V> {
    fn default() -> Self {
        Appareils::new()
    }
}

impl<V: fmt::Debug> fmt::Debug for Appareils<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.lignes.iter()).finish()
    }
}

impl<V: Forme> Forme for Appareils<V> {
    const EMPREINTE: u64 = forme::de("Appareils", V::EMPREINTE);
    const SECRET: Contient = V::SECRET;
    const VIDE: Contient = Contient::Non;
}

impl<V: Etat> Etat for Appareils<V> {
    fn projeter(&self, r: &Regard) -> Self {
        Appareils {
            lignes: self.lignes.projeter(r),
        }
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        self.lignes.ecrire(e);
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        let lignes = Vec::<(Appareil, V)>::lire(l)?;
        // Un appareil en double serait une forme qu'aucune table n'écrit.
        for (i, (a, _)) in lignes.iter().enumerate() {
            if lignes.iter().skip(i.saturating_add(1)).any(|(b, _)| b == a) {
                return Err(l.erreur(jeu_abi::Illisible::Valeur));
            }
        }
        Ok(Appareils { lignes })
    }
}

impl<V: Message> serde::Serialize for Appareils<V> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.lignes.serialize(s)
    }
}

impl<'de, V: Message> serde::Deserialize<'de> for Appareils<V> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let lignes = Vec::<(Appareil, V)>::deserialize(d)?;
        for (i, (a, _)) in lignes.iter().enumerate() {
            if lignes.iter().skip(i.saturating_add(1)).any(|(b, _)| b == a) {
                return Err(serde::de::Error::custom("un appareil en double"));
            }
        }
        Ok(Appareils { lignes })
    }
}

impl<V: Message> Message for Appareils<V> {}
