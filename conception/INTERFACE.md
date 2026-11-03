# Interface d'un jeu

> **Statut.** Conception retenue pour la première pièce de la reconstruction : l'API Rust qu'écrit l'auteur d'un jeu, le contrat binaire entre le module WebAssembly d'un jeu et le commun, et le découpage du commun en crates pour la phase 1. [`MANIFESTE.md`](../MANIFESTE.md) et [`CONTRAT.md`](../CONTRAT.md) font foi : ce document se corrige s'il les contredit.
>
> **Sources.** Les trois propositions de `conception/`, les trois `games/*/CAHIER.md` et les leçons de `lecons/sources/` (identifiants `PUN-`, `SQ-`, `TL-`, `SAL-`, `REM-`, `OUT-`, `HIS-`, `CAT-`).
>
> **Mesures.** Aucun chiffre de ce document n'est mesuré. Chaque budget est un point de départ, à mesurer puis à écrire dans `CONTRAT.md` avec sa date.

## 0. En une page

1. **Le module n'importe rien.** Il exporte sa mémoire et deux fonctions. Constater ce à quoi le jeu a accès, c'est lire une section d'imports vide.
2. **Une seule porte.** Chaque appel est une `Appel` encodée, chaque réponse un `Retour`. Les deux types ont une seule définition, dans la crate `jeu-abi`.
3. **Une seule fonction d'étape, en deux temps.** Le jeu lit l'état, puis accepte ou refuse. Il ne peut écrire qu'après avoir accepté, et ne peut refuser qu'avant. Le compilateur fait respecter « un refus ne change rien ». Le hasard et les sorties ne s'atteignent qu'après acceptation.
4. **Les lectures ne touchent pas à l'état.** Une vue, un bot, un invariant alloue dans une zone de brouillon, remise à zéro après l'appel. Le moteur le constate par empreinte.
5. **Ce que le jeu montre, demande et attend se lit dans l'état.** Les vues, les effets (avec leur identité et leur instant), les demandes en cours, le prochain réveil, qui la partie attend. Une demande qui disparaît de l'état est annulée ; sa réponse ne revient jamais.
6. **Le temps est un numéro de pas.** 60 pas par seconde. Le jeu ne voit ni heure pour décider, ni image. Une échéance ne se crée que depuis l'instant de l'entrée en cours.
7. **Un geste dit quand et sur quoi.** Où il est placé, quand il a été fait, quand il a été reçu, quelle vue l'appareil montrait, et son identité.
8. **Un secret est un `Secret<T>`.** L'étape l'ouvre avec le jeton `Autorite`. Une vue ou un bot ne le voit qu'à travers son `Regard`. La télé regarde à travers le sien, sans privilège.
9. **Une seule façon de repartir.** Restaurer une mémoire, puis rejouer le journal. Elle sert à la faute isolée, au geste en retard dans la fenêtre, au rechargement d'un onglet, à la prédiction et au rejeu d'un bug.
10. **Le cœur de la partie qui fait foi est une fonction de ses entrées.** Il ne connaît ni horloge, ni réseau, ni WebAssembly : il appelle une `Instance` abstraite. Il se teste à vitesse machine et se prête à la preuve.

## 1. D'où vient cette conception

La base est la proposition « temps réel ». Elle est la seule à écrire la règle du contrat telle quelle (« le jeu lit l'ancien état, et ne produit un nouvel état que s'il accepte »), à porter un FPS (fenêtre, appuis comptés, identité des gestes, résultat définitif) et à éviter une copie de mémoire avant chaque entrée.

Y sont greffés :

- de « plus petit noyau » : aucun import, une seule porte, les lectures comme appels sans effet, les demandes et les effets lus dans l'état, `no_std`, le réveil lu dans l'état, le compteur de profondeur ;
- de « types » : `Appareil` non ordonné, `Maintenant` qu'on ne peut pas garder et `Echeance` qui n'en naît que depuis lui, `Texte<L>` dont la limite a une seule définition, `Applicable` avec sa `Raison`, le hasard caché aux appareils, l'accord de la déclaration et des types vérifié à la compilation, la crate du module générée hors du jeu, une crate d'ABI sans type de jeu.

Ce qui a été écarté, et pourquoi, est dans le tableau des décisions (§ 12).

---

## 2. L'API Rust

L'auteur d'un jeu n'utilise que `jeu`. `jeu` réexporte le vocabulaire du commun défini dans `jeu-abi` (§ 8.1) : tout ce qui suit s'importe par `use jeu::prelude::*;`.

### 2.1 La forme d'un jeu

- La crate de règles du jeu est une bibliothèque `#![no_std]`, `#![forbid(unsafe_code)]`, avec `extern crate alloc`. Ses tests ont `std`.
- `no_std` fait de quatre règles d'écriture des formes : `HashMap` et `HashSet` n'existent pas, `Instant::now` et `SystemTime::now` non plus, ni les fonctions flottantes de la plateforme. Les mathématiques passent par `jeu::math`, qui réexporte `libm` (HIS-50, OUT-144, OUT-145).
- Le module est une seconde crate, `<jeu>-module`, de deux lignes, écrite par `soiree new` et vérifiée par `soiree check` :

```rust
#![no_std]
jeu::module!(punchline::Punchline);
```

  C'est elle seule qui porte le code `unsafe` des exports, dans la macro du commun.
- La page téléphone est une troisième crate, `<jeu>-page`, qui dépend des mêmes types que les règles (phase 2, § 6.6).

### 2.2 Messages et état

```rust
/// Ce qui traverse une frontière : vue, geste, avis, garde, résumé.
/// Seule la macro `#[jeu::message]` l'implémente.
pub trait Message: Etat + serde::Serialize + serde::de::DeserializeOwned + Forme {}

/// Ce qui peut vivre dans l'état. Seules `#[jeu::message]` et `#[jeu::etat]` l'implémentent,
/// en plus des types de `jeu`. Aucun type à mutabilité intérieure ne l'implémente.
pub trait Etat: Clone + core::fmt::Debug + Forme + 'static {
    /// La même valeur, chaque secret que `r` ne voit pas vidé.
    fn projeter(&self, r: &Regard) -> Self;
    #[doc(hidden)] fn ecrire(&self, e: &mut cache::Ecrivain<'_>);
    #[doc(hidden)] fn lire(l: &mut cache::Liseur<'_>) -> Result<Self, cache::Illisible>;
}

/// La forme d'un type, calculée à la compilation.
pub trait Forme {
    const EMPREINTE: u64;          // FNV-1a des noms, des champs et des formes des champs
    const SECRET: Contient;        // le type contient-il un `Secret` ?
    const VIDE: Contient;          // le type est-il `Aucun` ?
}
pub enum Contient { Oui, Non }
```

| Macro | Sur | Ce qu'elle écrit | Ce qu'elle refuse à la compilation |
|---|---|---|---|
| `#[jeu::message]` | struct, enum | `Clone`, `Debug`, `PartialEq`, `Serialize`, `Deserialize`, `Forme`, `Etat`, `Message` | un champ qui n'est pas `Message` ; les attributs `serde` `untagged`, `tag`, `content`, `flatten`, `default`, `skip*`, `with`, `other` (REM-60, PUN-91, SAL-119) |
| `#[jeu::etat]` | struct, enum | `Clone`, `Debug`, `Forme`, `Etat` (projection, forme canonique) | un champ qui n'est pas `Etat` |
| `#[derive(Variantes)]` | enum sans champ | `const TOUTES: &'static [Self]` | un variant à champs |

`Etat` est implémenté par `jeu` pour : les entiers, `bool`, `char`, `f32`, `f64`, `String`, `Vec<T>`, `Box<T>`, `Option<T>`, les tableaux, les tuples jusqu'à 8, `BTreeMap<K: Message + Ord, V>`, `BTreeSet<K: Message + Ord>`, `Partage<T>`, `Appareils<V>`, `Secret<T>`, et tout le vocabulaire du commun. `Message` l'est pour les mêmes types sauf `Secret`, `Partage` et les flottants nus.

- `Secret<T>` n'est pas `Message` : une vue, un avis, un résumé ne peuvent pas en contenir. Ils ne compilent pas.
- La forme canonique (`ecrire`, `lire`) est scellée : `Ecrivain` et `Liseur` ne se construisent que dans `jeu`. Le code du jeu ne peut pas sérialiser un secret pour en lire les octets.
- La forme canonique compte les flottants non finis qu'elle écrit. L'épreuve en fait une loi commune (CAT-05, SQ-68).
- `Debug` d'un secret écrit `<secret>`.

```rust
/// Le type sans valeur. `match x {}` suffit.
pub enum Aucun {}

/// Un contenu immuable partagé entre plusieurs copies de l'état, comme une image dans
/// l'historique de Tier list (TL-20). La forme canonique l'écrit une fois par allocation.
pub struct Partage<T: Etat>(/* Rc<T> */);
impl<T: Etat> Partage<T> {
    pub fn new(v: T) -> Self;
    pub fn get(&self) -> &T;
}
```

### 2.3 Les valeurs venues de l'extérieur

Chaque type qui franchit une frontière se valide à son décodage. Une valeur invalide ne se décode pas : elle n'atteint jamais l'étape, et ne coûte qu'à son lien (SAL-117).

```rust
/// Un f32 fini, sans sous-normal (ramené à 0). Ni NaN ni infini.
pub struct Fini(f32);
impl Fini { pub const ZERO: Fini; pub fn new(x: f32) -> Option<Fini>; pub fn get(self) -> f32; }

/// Un axe dans [-1, 1].
pub struct Axe(Fini);
/// Deux axes de norme au plus 1, zone morte de l'appareil déjà appliquée par le commun.
pub struct Axe2 { x: Axe, y: Axe }
impl Axe2 { pub const ZERO: Axe2; pub fn x(self) -> f32; pub fn y(self) -> f32; }

/// Un entier dans [MIN, MAX], vérifié au décodage.
pub struct Borne<const MIN: i64, const MAX: i64>(i64);
impl<const MIN: i64, const MAX: i64> Borne<MIN, MAX> {
    pub const PLUS_PETIT: Self;
    pub const fn new(v: i64) -> Option<Self>;
    pub const fn get(self) -> i64;
    pub const fn indice(self) -> usize;            // v - MIN, saturé
}

/// Un compteur qui ne fait que monter, absolu, en boucle sur u32 (appuis, visée cumulée).
/// Un paquet perdu ne décale rien (REM-13).
pub struct Cumul(u32);
impl Cumul { pub fn depuis(self, avant: Cumul) -> u32; }   // différence en boucle

/// Un bouton tenu : son état, et le nombre d'appuis depuis l'arrivée de l'appareil.
/// Un appui plus court qu'un envoi laisse une trace ; deux appuis comptent deux (REM-16, REM-17).
pub struct Bouton { pub etat: Appui, pub appuis: Cumul }
pub enum Appui { Tenu, Relache }

/// Une manette standard.
pub struct Manette { pub sticks: [Axe2; 2], pub gachettes: [Borne<0, 1000>; 2], pub boutons: [Bouton; 16] }

/// Un oui ou non, partout où un `bool` en paramètre serait permuté sans bruit.
pub enum Oui { Oui, Non }

/// Un court texte du commun (nom de fichier, type, prénom proposé, nom de flux) : UTF-8,
/// 64 octets au plus, sans caractère de contrôle. Le jeu le nettoie en `Texte<L>` s'il le garde.
pub struct TexteCourt { /* [u8; 64], u8 */ }
impl TexteCourt { pub const fn new(s: &'static str) -> TexteCourt; pub fn as_str(&self) -> &str; }
```

**Les textes.** Un texte venu d'un appareil n'est jamais une `String` : il porte sa limite dans son type.

```rust
/// Une limite de texte : une seule définition, pour les règles et pour le champ qui la saisit
/// (TL-26, TL-27, SQ-40).
pub trait Limite: 'static {
    const MAX: u32;
    const UNITE: Unite;
    const ENCRE: Encre;
}
pub enum Unite { Capitales, Utf16, PointsDeCode, Graphemes }
/// Ce qu'une police dessine. `Toute` tant qu'aucune table n'est fournie.
pub enum Encre { Toute, Table(&'static [(u32, u32)]) }

/// `jeu::limite!(pub Prenom = 12 Capitales);`
/// `jeu::limite!(pub Message = 80 PointsDeCode, encre ARCADE);`
#[macro_export] macro_rules! limite { /* … */ }

/// Un texte nettoyé : NFC ; contrôles, points de code ignorables et catégorie Cf retirés ;
/// liants et sélecteurs gardés seulement dans un emoji ; espaces réduites ; coupé entre
/// deux graphèmes, au plus `L::MAX` unités de `L::UNITE` (PUN-22 à PUN-26, CAT-79 à CAT-85, TL-28).
pub struct Texte<L: Limite> { /* String, PhantomData<L> */ }
impl<L: Limite> Texte<L> {
    /// Le seul nettoyage. Nettoyer deux fois revient à nettoyer une fois.
    pub fn nettoyer(brut: &str) -> Texte<L>;
    pub fn as_str(&self) -> &str;
    pub fn vide(&self) -> Oui;
    /// La seule clé de comparaison du commun : casse ignorée, sans sélecteurs ni liants (CAT-117).
    pub fn cle(&self) -> CleTexte;
    pub fn poids(&self) -> u32;                         // en unités de L::UNITE
    pub fn encre(&self) -> Encrable;                    // Tout | Partiel | Rien
    /// Ajoute un suffixe, et raccourcit entre graphèmes pour qu'il tienne (PUN-25).
    pub fn avec_suffixe(&self, suffixe: &str) -> Texte<L>;
    pub fn borne<M: Limite>(&self) -> Option<Texte<M>>;
}
pub struct CleTexte(/* String */);                      // Ord, Eq
pub enum Encrable { Tout, Partiel, Rien }
```

Au décodage, un `Texte<L>` n'est accepté que s'il est déjà nettoyé et sous sa limite. La page téléphone le nettoie avec la même fonction, dans la même crate : une page honnête n'est jamais refusée, une page hostile l'est toujours. Le nettoyage embarque `unicode-normalization` et `unicode-segmentation` dans le module : une version du module a une seule version d'Unicode, et un rejeu ne change pas.

### 2.4 Le temps

```rust
pub const PAS_PAR_SECONDE: u64 = 60;

/// Un instant passé de la partie, en pas. Se garde dans l'état. Ne s'additionne pas.
pub struct Pas(u64);                                     // Copy, Ord
/// Un instant à l'intérieur d'un pas, en 1/65 536 de pas, pour départager une course serrée.
pub struct Instant { pub pas: Pas, pub fraction: u16 }   // Copy, Ord
impl Instant { pub const fn de(p: Pas) -> Instant; }

/// Une durée en pas entiers. Jamais une somme de flottants (CAT-15).
pub struct Duree(u64);                                   // Copy, Ord
impl Duree {
    pub const fn pas(n: u64) -> Duree;
    pub const fn ms(n: u64) -> Duree;                    // arrondi au pas supérieur : 450 ms = 27 pas
    pub const fn s(n: u64) -> Duree;
    pub const fn en_pas(self) -> u64;
    pub const fn en_ms(self) -> u64;
    pub const fn secondes_restantes(self) -> u64;        // arrondi au-dessus (SQ-35)
    pub const fn plus(self, d: Duree) -> Duree;          // saturé
}

/// L'heure UTC de l'ordinateur de la soirée, en ms, enregistrée au journal.
/// Pour étiqueter une ligne de résultats, jamais pour décider.
pub struct HeureMurale(u64);

/// L'instant de l'entrée en cours. Ni `Etat`, ni `Clone` : il ne se garde pas.
pub struct Maintenant<'e> { /* Pas, PhantomData<&'e ()> */ }
impl Maintenant<'_> {
    pub fn pas(&self) -> Pas;
    pub fn depuis(&self, p: Pas) -> Duree;               // saturé à zéro
}

/// Une échéance. Se garde dans l'état. Ne naît que depuis `Maintenant` (PUN-34).
pub struct Echeance(Pas);                                // Copy, Ord, Etat, Message
impl Echeance {
    pub fn dans(m: &Maintenant<'_>, d: Duree) -> Echeance;
    pub fn atteinte(&self, m: &Maintenant<'_>) -> Oui;
    pub fn atteinte_a(&self, p: Pas) -> Oui;             // pour `quand.recu` d'un brouillon
    pub fn restant(&self, m: &Maintenant<'_>) -> Duree;
    /// Repousse d'une durée mesurée, comme une pause (Sidequest, cahier 4.8).
    pub fn repoussee(self, d: Duree) -> Echeance;
    pub fn pas(&self) -> Pas;
}

/// Quand le jeu veut recevoir le temps. Lu après chaque entrée acceptée.
pub enum Reveil {
    /// Un pas à chaque pas (temps réel, glisser de Tier list).
    ChaquePas,
    /// Un seul pas, à l'échéance, ou au réveil si elle est passée.
    A(Echeance),
    /// Aucun pas : le jeu n'avance que sur des entrées.
    Jamais,
}
```

Il n'existe ni `Pas + Duree`, ni `Echeance + Duree`. Une phase qui suit une autre part de `Maintenant`, jamais de l'échéance d'avant : après un gel, une seule phase se termine, et la suivante reçoit toute sa durée (PUN-34). `repoussee` reste possible pour une pause : la forme oriente, la pression d'un gel injecté juge (§ 7).

### 2.5 Appareils et présence

```rust
/// Un appareil, créé par le commun seul. Ni `Ord`, ni `Hash`, ni constructeur public.
/// Ses valeurs sont tirées au hasard pour chaque soirée : elles ne départagent rien (CAT-27).
pub struct Appareil(u64);                                // Copy, Eq, Message

/// Une table d'appareils, dans l'ordre où ils y sont entrés.
pub struct Appareils<V> { /* Vec<(Appareil, V)> */ }
impl<V> Appareils<V> {
    pub const fn new() -> Self;
    pub fn get(&self, a: Appareil) -> Option<&V>;
    pub fn get_mut(&mut self, a: Appareil) -> Option<&mut V>;
    pub fn inserer(&mut self, a: Appareil, v: V) -> Option<V>;   // remplace sur place, sinon à la fin
    pub fn retirer(&mut self, a: Appareil) -> Option<V>;
    pub fn iter(&self) -> impl Iterator<Item = (Appareil, &V)>;
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Appareil, &mut V)>;
    pub fn len(&self) -> usize;
}

pub enum Presence {
    /// Un appareil jamais vu, ou oublié. Ses capacités présentes, pas seulement leurs changements (REM-43).
    Arrivee { appareil: Appareil, role: RoleAppareil, capacites: Capacites },
    Depart { appareil: Appareil, cause: CauseDepart },
    Retour { appareil: Appareil, capacites: Capacites },
    Capacites { appareil: Appareil, capacites: Capacites },
}
pub enum RoleAppareil { Telephone, Manette }
pub enum CauseDepart { Silence, Fermeture, Voulu }

/// Des capacités, jamais une technologie. Toutes optionnelles.
pub struct Capacites {
    pub vibration: Capacite,
    pub mouvement: Capacite,          // capteurs, dans les axes de l'interface (REM-137)
    pub son_sans_geste: Capacite,
    pub ecran_allume: Capacite,
    pub micro: Capacite,              // jamais sur une adresse locale en http (OUT-211)
    pub pad_natif: Capacite,
    pub orientation: Orientation,     // telle qu'elle est, demi-tour compris (REM-44)
    pub prenom_propose: Option<TexteCourt>,   // 64 octets au plus, à nettoyer par le jeu
}
pub enum Capacite { Oui, Non(Absence) }
pub enum Absence { NonSupportee, ContexteNonSecurise, Refusee, PasEncore }
pub enum Orientation { Portrait, PortraitInverse, PaysageGauche, PaysageDroite }
```

L'écran de l'ordinateur de la soirée n'est pas un `Appareil` : il est local, toujours là, et ses entrées ont leur propre type (§ 2.6). Un téléphone ne peut pas l'usurper (SAL-33, SQ-21, PUN-48). Le jeu ne voit ni lien, ni session, ni onglet, ni jeton. Un onglet jumeau, un rechargement, une app et sa page sont un seul appareil (SAL-15, SAL-154, SQ-88, SQ-89). Le commun ne libère jamais une place seul : il annonce, et le jeu décide (§ 2.7, `oublier`, `retirer`).

### 2.6 Les entrées

```rust
pub enum Entree<'a, J: Jeu> {
    /// Le temps : le pas en cours, livré seulement si `reveil()` le demande.
    Temps(Temps),
    /// L'écran de l'ordinateur de la soirée : sa scène, son clavier, sa souris.
    Ecran(DeEcran<'a, J::ActionEcran>),
    /// Un événement d'un téléphone : compté exactement une fois, dans l'ordre fixé.
    Geste(Geste<J::GesteTelephone>),
    /// Le changement d'un état tenu d'un téléphone : le dernier gagne.
    Tenu(Changement<J::TenuTelephone>),
    /// Le changement de l'état d'une manette.
    Manette(Changement<Manette>),
    Presence(Presence),
    /// La réponse à une demande encore listée par le jeu. Jamais pour une demande retirée.
    Reponse(IdDemande, Reponse<'a>),
}

pub struct Temps {
    /// Le temps jeté après un gel, au-delà du rattrapage déclaré. `Duree::pas(0)` d'habitude.
    pub saut: Duree,
    pub heure: HeureMurale,
}

pub struct Geste<G> { pub de: Appareil, pub geste: G, pub quand: Quand }
pub struct IdGeste(u64);                                 // Copy, Eq, Ord, Message
pub struct IdFichier(u64);                               // Copy, Eq, Ord, Message

/// Quand et sur quoi un geste a été fait.
pub struct Quand {
    /// Le pas où le commun l'applique : celui de l'entrée en cours.
    pub place: Pas,
    /// L'instant estimé du geste, sur l'horloge de la partie, borné (§ 4.3).
    pub fait: Instant,
    /// Le pas où la partie qui fait foi l'a reçu.
    pub recu: Pas,
    /// Le pas de la dernière vue que l'appareil avait reçue. Borné par ce qui lui a été envoyé.
    pub vu: Pas,
    /// L'identité du geste, stable à travers renvois, rejeux et prédiction (§ 5.4).
    pub id: IdGeste,
}

pub struct Changement<T: Tenu> { pub de: Appareil, pub valeur: T, pub cause: CauseTenu, pub quand: Quand }
pub enum CauseTenu { Geste, Silence, Depart, ArrierePlan, Pause }

/// Un état tenu : complet, le dernier gagne, un par clé et par appareil.
pub trait Tenu: Message + PartialEq {
    type Cle: Message + Ord + Copy;
    fn cle(&self) -> Self::Cle;
    /// Cette valeur relâchée (le stick au centre, le bouton levé, compteur d'appuis gardé).
    /// `None` : ne se relâche jamais seul (un brouillon).
    fn relache(&self) -> Option<Self>;
}
impl Tenu for Aucun { /* … */ }
impl Tenu for Manette { /* Cle = (), relâche sticks et boutons, garde les Cumul */ }

pub enum DeEcran<'a, A> {
    /// Un bouton ou un raccourci que la scène affichée portait. Rien d'autre ne s'envoie (SAL-152).
    Action { action: A, quand: Quand },
    /// Une touche brute, seulement si la scène l'a demandée (`clavier_brut`).
    Touche { touche: Touche, quand: Quand },
    /// Un pointeur brut, sur une surface que la scène a déclarée.
    Pointeur { pointeur: Pointeur, quand: Quand },
    /// Le résultat d'un composant de la scène : texte saisi, objet déposé, fichiers reçus.
    Composant { evenement: Composant<'a>, quand: Quand },
}
```

Les types des entrées de l'écran appartiennent au commun :

```rust
pub struct Touche { pub code: CodeTouche, pub mods: Mods, pub phase: PhaseTouche, pub repetition: Oui }
/// Le nom de la touche physique, tel que le navigateur le donne (« KeyW », « Space ») : du texte,
/// pas un contrat entre nos fichiers (CONTRAT, « Les types de bout en bout »). 24 octets ASCII au plus.
pub struct CodeTouche { /* [u8; 24], u8 */ }
impl CodeTouche { pub const fn new(nom: &'static str) -> CodeTouche; pub const ESPACE: CodeTouche; /* … */ }
pub struct Mods { pub maj: Oui, pub ctrl: Oui, pub alt: Oui, pub meta: Oui }
impl Mods { pub const AUCUN: Mods; pub const MAJ: Mods; pub const CMD_OU_CTRL: Mods; }
pub enum PhaseTouche { Enfoncee, Relachee }

pub struct Pointeur { pub id: u32, pub genre: GenrePointeur, pub bouton: u8, pub phase: PhasePointeur, pub pos: (Fixe, Fixe) }
pub enum GenrePointeur { Souris, Doigt, Stylet }          // un seul chemin (TL-36)
pub enum PhasePointeur { Appui, Deplacement, Relache, Annule }

pub enum Composant<'a> {
    /// Jamais pendant une composition IME (TL-32, PUN-70).
    Saisie { champ: Cle, texte: &'a str, fin: FinSaisie },
    Deplace(Deplace),
    Fichiers { geste: IdGeste, fichiers: Vec<FichierRecu> },
}
pub enum FinSaisie { EnCours, Entree, PerteDeFocus }
pub enum Deplace {
    /// La place visée, comptée dans l'ordre de la zone, sans l'objet tenu (TL-39).
    Survol { objet: Cle, zone: Cle, indice: u32 },
    Depose { objet: Cle, zone: Cle, indice: u32 },
    Annule { objet: Cle },                                // Échap, pointercancel
}
/// Un fichier choisi, déposé ou collé. Ses octets restent chez le commun jusqu'à une demande (TL-14).
pub struct FichierRecu { pub id: IdFichier, pub nom: TexteCourt, pub genre: TexteCourt, pub taille: u64 }
```

Le moteur valide chaque entrée de l'écran contre la dernière scène envoyée : une action doit y figurer, un champ y exister, un texte tenir dans sa limite, un objet et une zone y être. Sinon l'entrée n'atteint pas le jeu.

### 2.7 L'étape

C'est le cœur de l'interface. Une seule fonction reçoit toutes les entrées, une à une.

```rust
/// Avant la décision : lecture seule.
pub struct Avant<'e, J: Jeu> { /* privé */ }
impl<'e, J: Jeu> Avant<'e, J> {
    pub fn jeu(&self) -> &J;
    pub fn autorite(&self) -> &Autorite;
    pub fn maintenant(&self) -> Maintenant<'_>;
    /// Accepter donne l'écriture. On ne peut plus refuser ensuite.
    pub fn accepter(self) -> Apres<'e, J>;
    /// L'état ne change pas ; l'avis va au seul appareil du geste, sans changer sa vue.
    pub fn refuser(self, avis: J::Avis) -> Issue<J>;
    /// L'état ne change pas ; personne n'est prévenu.
    pub fn ignorer(self) -> Issue<J>;
    /// Un secret manque : sur un appareil qui prédit, ce geste attend la confirmation.
    /// Sur la partie qui fait foi, c'est une faute du jeu.
    pub fn attendre(self, a: Absent) -> Issue<J>;
}

/// Après la décision : écriture.
pub struct Apres<'e, J: Jeu> { /* privé */ }
impl<'e, J: Jeu> Apres<'e, J> {
    /// Tout ce que l'écriture demande, emprunté ensemble.
    pub fn parts(&mut self) -> Parts<'_, J>;
    pub fn jeu(&mut self) -> &mut J;
    pub fn fin(self) -> Issue<J>;
    pub fn attendre(self, a: Absent) -> Issue<J>;
}
pub struct Parts<'p, J: Jeu> {
    pub jeu: &'p mut J,
    pub autorite: &'p Autorite,
    pub maintenant: Maintenant<'p>,
    pub sorties: &'p mut Sorties,
}
impl Sorties {
    /// Le générateur de la partie. C'est un secret : il manque sur un appareil, et un tirage
    /// y arrête la prédiction de ce geste. Un jeu qui déclare `hasard: SansObjet` et l'appelle commet une faute.
    pub fn hasard(&mut self) -> Result<&mut Hasard, Absent>;
    /// Un identifiant neuf, jamais redonné dans la soirée, même par une annulation du jeu (TL-23).
    pub fn id(&mut self) -> IdDemande;
    /// Oublie un appareil parti : il reviendra comme un neuf (PUN-15, SQ-76).
    pub fn oublier(&mut self, a: Appareil);
    /// Retire un appareil présent : il lit « Tu as été retiré », et peut revenir comme un neuf.
    pub fn retirer(&mut self, a: Appareil);
    /// Une nouvelle partie : une instance neuve, avec `garder()` et les mêmes appareils (CAT-73, CAT-75).
    pub fn nouvelle_partie(&mut self);
}

/// Le résultat d'une étape. Sans constructeur public : on ne l'obtient qu'en refusant,
/// en ignorant, en attendant ou en finissant.
pub struct Issue<J: Jeu> { /* privé */ }
```

- `refuser` et `accepter` consomment `Avant`. Après `accepter`, refuser ne s'écrit plus.
- Avant d'accepter, le jeu n'a que `&J` : il ne peut ni écrire l'état, ni tirer du hasard, ni émettre une sortie. Un refus ne change rien, générateur compris, par la forme (PUN-27, HIS-44, OUT-171, CAT-94, SQ-25, TL-02).
- Une allocation faite avant la décision touche l'allocateur, pas l'état du jeu. Le journal garde les entrées refusées, et un rejeu les refait : la mémoire reste identique d'une exécution à l'autre (§ 4.5).
- Un refus d'une entrée de l'écran va à l'écran : le commun l'affiche dans la zone `avertissements` de la scène. Un refus d'une entrée sans origine (`Temps`, `Presence`, `Reponse`) vaut `ignorer`, et l'épreuve le signale comme une faute du jeu.
- `#[jeu::etat]` refuse tout champ à mutabilité intérieure. Un `static` à mutabilité intérieure dans la crate du jeu est refusé par `soiree check`. L'épreuve constate qu'un refus et une lecture laissent l'état intact (§ 7.2).

```rust
/// Le générateur du jeu : xoshiro256**, semé par une graine de 256 bits, neuve à chaque soirée
/// sauf rejeu voulu (PUN-89, CAT-92, CAT-93). Il vit dans l'enveloppe du module, en secret.
pub struct Hasard { /* [u64; 4] */ }
impl Hasard {
    pub fn entier(&mut self) -> u64;
    pub fn borne(&mut self, n: core::num::NonZeroU64) -> u64;      // sans biais, méthode de Lemire (CAT-91)
    pub fn dans<const MIN: i64, const MAX: i64>(&mut self) -> Borne<MIN, MAX>;
    pub fn melanger<T>(&mut self, v: &mut [T]);                    // Fisher-Yates (CAT-89, CAT-90)
    pub fn choisir<'v, T>(&mut self, v: &'v [T]) -> Option<&'v T>;
    pub fn une_sur(&mut self, n: core::num::NonZeroU32) -> Oui;
    pub fn deriver(&mut self) -> Hasard;
}
```

### 2.8 Les secrets

```rust
/// Le jeton qui ouvre un secret. Seuls l'étape, la création et les invariants le reçoivent.
/// Ni `Clone`, ni `Copy`, ni constructeur public.
pub struct Autorite { /* privé */ }

/// Qui regarde : l'écran, ou un téléphone. Construit par le commun seul.
pub struct Regard { /* privé */ }
impl Regard {
    pub fn ecran(&self) -> Oui;
    pub fn appareil(&self) -> Option<Appareil>;
}

/// Une valeur que tout le monde ne voit pas.
pub struct Secret<T: Etat> { /* qui: Qui, valeur: Option<T> */ }
/// Qui voit un secret. Cette information est publique : on sait qu'un secret existe, et qui le voit.
pub enum Qui { Personne, Appareil(Appareil), Appareils(Lecteurs), Ecran, Tous }
pub struct Lecteurs(/* 8 appareils au plus */);

/// Le secret n'est pas lisible ici. `?` le convertit dans l'issue `attendre`.
pub struct Absent;

impl<T: Etat> Secret<T> {
    pub fn cacher(qui: Qui, valeur: T) -> Secret<T>;
    pub fn lire(&self, a: &Autorite) -> Result<&T, Absent>;
    pub fn lire_mut(&mut self, a: &Autorite) -> Result<&mut T, Absent>;
    /// Pour une vue ou un bot : seulement si ce regard a le droit.
    pub fn voir(&self, r: &Regard) -> Option<&T>;
    /// Change qui voit. Une révélation est un changement d'état, fait par l'étape.
    pub fn montrer(&mut self, qui: Qui);
    pub fn qui(&self) -> &Qui;
}
```

| Porte | Qui l'a | Ce qu'elle ouvre |
|---|---|---|
| `lire`, `lire_mut` avec `Autorite` | l'étape, la création, `resume`, `invariants` | tout, sur la partie qui fait foi ; seulement ce que la projection contient, sur un appareil |
| `voir` avec `Regard` | les vues, les bots | ce que ce regard voit : `Tous` et `Ecran` pour l'écran ; `Tous`, son appareil et ses `Lecteurs` pour un téléphone |
| `montrer` | l'étape, après `accepter` | change qui voit |

La télé est dessinée sur la machine qui détient tous les secrets, mais elle les regarde à travers le regard `Ecran` : elle ne peut pas montrer un auteur avant la révélation (CAT-46). La forme d'un secret fuit (qu'il existe, qui le voit) : pour cacher combien il y a de réponses, on cache la liste entière.

### 2.9 Les vues

```rust
/// Ce qu'un téléphone reçoit : sa vue, ses effets, ses réglages.
#[jeu::message]
pub struct Montre<V: Message> { pub vue: V, pub effets: Vec<Effet>, pub reglages: Reglages }
#[jeu::message]
pub struct Reglages { pub ecran_allume: Oui, pub fond: Option<Rgba>, pub orientation: Option<Orientation> }

/// Une valeur d'une vue de téléphone qui ne s'affiche qu'une fois définitive (prédiction, § 5).
#[jeu::message]
pub struct Definitif<T: Message> { /* cle: Cle, valeur: Option<T> */ }
impl<T: Message> Definitif<T> {
    pub fn de(cle: Cle, valeur: T) -> Definitif<T>;
    pub fn valeur(&self) -> Option<&T>;                 // sur la page : `None` tant que ce n'est pas sûr
}

/// Une clé stable, calculée depuis une valeur du jeu, jamais depuis une position (SAL-165).
pub struct Cle(u64);
impl Cle { pub fn de(v: &impl serde::Serialize) -> Cle; }  // FNV-1a 64 de l'encodage postcard
```

**La scène de l'écran** est le langage de dessin du commun. Elle est une donnée, recalculée depuis l'état, et dessinée par le commun dans la zone sûre (HIS-46, PUN-51). Ses longueurs sont des `Fixe`, entiers en 1/256 d'unité du repère : NaN et l'infini n'y sont pas représentables (REM-57).

```rust
pub struct Fixe(i32);
impl Fixe { pub const fn unites(n: i32) -> Fixe; pub fn depuis_f32(x: f32) -> Fixe; } // sature ; NaN → 0, compté par l'épreuve
pub struct Rect { pub x: Fixe, pub y: Fixe, pub l: Fixe, pub h: Fixe }
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

pub struct Scene<A: Message> { /* privé */ }
impl<A: Message> Scene<A> {
    pub fn fond(&mut self, c: Rgba);
    pub fn rect(&mut self, cle: Cle, r: Rect, couleur: Rgba) -> &mut Noeud;
    pub fn texte(&mut self, cle: Cle, texte: &str, boite: Rect, style: StyleTexte) -> &mut Noeud;
    pub fn image(&mut self, cle: Cle, image: Cle, r: Rect, ech: Echantillonnage) -> &mut Noeud;
    pub fn groupe(&mut self, cle: Cle, decalage: (Fixe, Fixe), f: impl FnOnce(&mut Scene<A>)) -> &mut Noeud;
    pub fn bouton(&mut self, cle: Cle, boite: Rect, nom: &str, action: A) -> &mut Noeud;
    pub fn raccourci(&mut self, touche: CodeTouche, mods: Mods, repetition: Oui, action: A);
    pub fn compte_a_rebours(&mut self, cle: Cle, c: CompteARebours, boite: Rect, style: StyleTexte) -> &mut Noeud;
    pub fn qr(&mut self, cle: Cle, boite: Rect) -> &mut Noeud;   // l'adresse que les téléphones joignent vraiment (SQ-16)
    pub fn composant(&mut self, cle: Cle, c: ComposantVu<A>) -> &mut Noeud;
    pub fn avertissements(&mut self, cle: Cle, boite: Rect);    // où le commun écrit les siens (TL-71)
    pub fn clavier_brut(&mut self, c: ClavierBrut);
    pub fn surface(&mut self, cle: Cle, r: Rect);                // pointeur brut, sans défilement natif
    /// Ce sous-arbre ne vient que de l'instance définitive, par clé (§ 4.4).
    pub fn definitif(&mut self, cle: Cle, f: impl FnOnce(&mut Scene<A>));
    pub fn effet(&mut self, e: Effet);
}
impl Noeud {
    pub fn glisse(&mut self) -> &mut Noeud;                       // glisse entre deux pas, par sa clé
    pub fn rupture(&mut self) -> &mut Noeud;                      // ne glisse pas à ce pas (TL-23)
    pub fn pixel_entier(&mut self) -> &mut Noeud;                 // SQ-52
    pub fn lisse(&mut self, d: Duree) -> &mut Noeud;              // lisse une correction de prédiction
    pub fn anime(&mut self, depuis: Instant, a: Animation) -> &mut Noeud;   // part d'un instant de la partie (REM-12)
    pub fn opacite(&mut self, o: u8) -> &mut Noeud;
}
pub enum CompteARebours { Court { echeance: Echeance }, Fige { restant: Duree } }   // dessiné par le commun (SQ-36, CAT-99)
pub struct StyleTexte { pub police: Cle, pub taille: Taille, pub couleur: Rgba, pub alignement: Alignement, pub lignes: Lignes, pub coupe: Coupe }
pub enum Taille { Fixe(Fixe), Budget { max: Fixe, min: Fixe } }  // rétrécit pour tenir (TL-55, PUN-53)
pub enum Lignes { Une, AuPlus(u8), Libre }
pub enum Coupe { Mots, Graphemes }                                // le signe isolé reste avec son mot (SQ-46)
pub enum Alignement { Debut, Centre, Fin }
pub enum Echantillonnage { Lisse, PlusProche }
pub struct Animation { pub duree: Duree, pub de: Visuel, pub a: Visuel, pub courbe: Courbe }
pub struct Visuel { pub decalage: (Fixe, Fixe), pub echelle_pour_mille: u16, pub opacite: u8 }
pub enum Courbe { Lineaire, Entree, Sortie, EntreeSortie, Rebond }
pub enum ClavierBrut { Non, Oui { repetition: Oui } }

pub enum ComposantVu<A> {
    Champ { limite: u32, unite: Unite, lignes: Lignes, valeur: String, exemple: String, fin: Option<A> },
    EnPlace { limite: u32, unite: Unite, valeur: String, style: StyleTexte },
    Deplacables { zones: Vec<ZoneVu>, poubelle: Option<Cle>, seuil: Fixe, bord: Fixe, vitesse_max: Fixe, plafond_reveil: Duree },
    Selecteur { genres: Vec<TexteCourt>, multiple: Oui, depot_partout: Oui, collage: Oui, taille_max: u64 },
    Modale { fermer: A },
    Lien { nom: TexteCourt, ressource: Cle },
}
pub struct ZoneVu { pub cle: Cle, pub objets: Vec<Cle> }
```

Une image de la scène est nommée par une `Cle`, jamais par une adresse : aucune requête ne sort (TL-15). Le moteur demande les octets qu'il n'a pas encore, une fois, par `Jeu::ressource`, ou dans les assets déclarés.

**Les effets** sont une ligne de la vue, comme le reste :

```rust
#[jeu::message]
pub struct Effet {
    /// Une identité sémantique : la même chose à jouer porte la même identité (PUN-66, SQ-79).
    pub id: IdEffet,
    /// L'instant où l'effet doit se produire, sur l'horloge de la partie.
    pub quand: Instant,
    pub quoi: Sensation,
    /// `Provisoire` se lit sur la tête, `Definitive` sur l'instance définitive (§ 4.4).
    pub tenue: Tenue,
    /// Au-delà de `quand` plus cette durée, l'effet ne se joue plus (SAL-155).
    pub peremption: Duree,
}
pub struct IdEffet(u64);
impl IdEffet { pub fn de(v: &impl serde::Serialize) -> IdEffet; }
pub enum Tenue { Provisoire, Definitive }
pub enum Sensation { Son(Son), Vibration(Vibration) }
pub enum Son { Signal(Signal), Asset(Cle) }
pub enum Signal { Debut, Avance, Resultat, Fin }           // synthétisés par le commun
pub enum Vibration { Courte, Succes }
```

### 2.10 Demandes et réponses

Une demande existe tant que le jeu la liste. Le moteur compare la liste d'une fois sur l'autre : il lance ce qui est nouveau, annule ce qui a disparu, et jette la réponse d'une demande qui n'est plus listée avant qu'elle n'atteigne le jeu (TL-44, TL-45, TL-46, SAL-164). Une demande ne part que de l'instance définitive : une ligne de résultats ne s'écrit jamais deux fois (CAT-76).

```rust
pub struct IdDemande(u64);                              // de `Sorties::id()` seulement
pub struct Demandes<'d> { /* privé */ }
impl<'d> Demandes<'d> {
    /// `f` n'est appelée que si le moteur ne connaît pas encore `id`.
    pub fn ajouter(&mut self, id: IdDemande, f: impl FnOnce() -> Demande<'d>);
}
pub enum Demande<'d> {
    /// Décoder, recadrer au centre, réduire au carré, encoder (TL-60).
    Image { fichier: IdFichier, carre: u16, format: FormatImage },
    /// Dessiner une scène hors écran, sans faire sauter d'image à l'écran (TL-57).
    Rendu { scene: Scene<Aucun>, echelle: u8, format: FormatImage },
    /// Lire un fichier reçu, taille vérifiée avant lecture (TL-14).
    Lire { fichier: IdFichier, max: u64 },
    /// Donner un fichier à télécharger.
    Telecharger { nom: &'d str, octets: &'d [u8] },
    /// Ajouter une ligne à un flux de résultats de l'ordinateur de la soirée.
    Ligne { flux: Flux, octets: &'d [u8] },
}
pub enum FormatImage { Png, WebpSinonPng { qualite: u8 } }
pub struct Flux(/* TexteCourt */);
impl Flux { pub const fn new(nom: &'static str) -> Flux; }

pub enum Reponse<'a> {
    Image(Result<&'a [u8], EchecDemande>),
    Rendu(Result<&'a [u8], EchecDemande>),
    Lu(Result<&'a [u8], EchecDemande>),
    Fait(Result<(), EchecDemande>),
}
pub enum EchecDemande { TropGros, Illisible, Refuse, Annule, Interne }
```

Une demande s'exécute au plus une fois par identité, même à travers une reprise : le moteur note l'identité avec sa réponse. Un échec revient comme une réponse ; le jeu l'annonce, et la partie continue (SQ-78).

### 2.11 Ce que le jeu dit de lui-même

```rust
/// Où en est la partie. L'épreuve vérifie qu'elle les parcourt toutes.
pub trait Phase: Message + Variantes + Copy + Eq {
    fn terminale(self) -> Oui;
}
pub trait Variantes: Sized + 'static { const TOUTES: &'static [Self]; }

/// Qui la partie attend, pour que le commun vérifie qu'elle ne peut pas se figer (CAT-78).
pub struct Attente<'a, J: Jeu> { /* privé */ }
impl<J: Jeu> Attente<'_, J> {
    /// Cet appareil peut faire avancer la partie ; l'un de ces gestes doit être accepté.
    pub fn joueur(&mut self, a: Appareil, exemples: &[J::GesteTelephone]);
    /// L'écran peut faire avancer la partie ; l'une de ces actions doit être acceptée.
    pub fn ecran(&mut self, exemples: &[J::ActionEcran]);
    /// La partie avancera seule à cette échéance.
    pub fn echeance(&mut self, e: Echeance);
    /// Rien n'avance seul, par nature (un éditeur).
    pub fn libre(&mut self, r: Raison);
}

pub enum Accueil { Ouverte, Pleine, Fermee }
pub enum Prevision { Oui, Non }
```

### 2.12 La déclaration

Chaque champ est obligatoire, `Declaration` n'a pas de `Default`. Un champ ajouté par une version du commun casse la compilation de chaque jeu, là où il faut répondre. Une exemption est une valeur avec sa raison, jamais une absence (HIS-04, HIS-43, OUT-03, OUT-04, OUT-05).

```rust
pub struct Declaration {
    pub nom: &'static str,
    pub places: Places,
    pub rattrapage: u8,              // pas rattrapés au plus après un gel (SQ-62)
    pub fenetre: Applicable<Duree>,
    pub antidatage_max: Duree,
    pub pause: Pause,
    pub ecran: FormeEcran,
    pub telephones: Applicable<Telephones>,
    pub manettes: Applicable<ReglesTenu>,
    pub prediction: Applicable<()>,
    pub hasard: Applicable<()>,
    pub secrets: Applicable<()>,
    pub fin: Applicable<()>,
    pub capacites: Voulues,
    pub limites: Limites,
    pub diagnostic: CodeTouche,
    pub assets: &'static [Asset],
    pub mecaniques: &'static [Mecanique],
}

pub enum Applicable<T> { Oui(T), SansObjet(Raison) }
/// Une raison qui dit quelque chose : au moins trois mots. Vérifiée à la compilation par `raison!`.
pub struct Raison(&'static str);
#[macro_export] macro_rules! raison { ($s:literal) => {{ const R: $crate::Raison = $crate::Raison::new($s); R }} }

pub struct Places { /* privé */ }
impl Places {
    /// Refuse à la compilation min > max, max = 0, plus de joueurs que d'appareils (OUT-200, HIS-13).
    pub const fn joueurs(min: u16, max: u16) -> Places;
    pub const fn appareils(self, max: u16) -> Places;
}
pub enum Pause { DuJeu, DuCommun, Aucune(Raison) }
pub enum FormeEcran {
    Scene { largeur: u16, hauteur: u16, zone_sure_pour_cent: u8, echelle_entiere: Oui },
    Page { largeur_max: u16 },                           // une page qui défile (Tier list)
}
pub struct Telephones { pub repartition: Repartition, pub tenu: ReglesTenu }
pub enum Repartition { UnParJoueur, UnParEquipe, UnPourLaPartie }
pub struct ReglesTenu {
    pub fraicheur: Applicable<Duree>,                    // relâché après ce silence (SQ-01)
    pub age_max: Applicable<Duree>,                      // un état plus vieux ne compte pas, un relâchement passe toujours (SQ-02)
    pub debit_max: u16,                                  // par seconde, par clé et par appareil (PUN-03)
}
pub struct Voulues { pub vibration: Oui, pub mouvement: Oui, pub son: Oui, pub ecran_allume: Oui, pub micro: Oui, pub pad_natif: Oui }
pub struct Limites {
    pub geste: u32,          // octets d'un geste ou d'un tenu
    pub file: u16,           // événements en attente par appareil
    pub vue: u32,            // octets d'une vue encodée
    pub effets: u16,         // effets par vue
    pub memoire: u32,        // pages de 64 Kio au plus
    pub brouillon: u32,      // octets de la zone de brouillon des lectures
}
pub struct Asset { pub cle: Cle, pub sorte: SorteAsset, pub octets: &'static [u8] }
pub enum SorteAsset { Police, Image, Son }
pub enum Mecanique { TempsReel, Physique, TourParTour, Vote, Texte, Hasard, Economie, Progression, Egalites, Affichage, Plateau, Fichiers }
```

Il n'existe pas de capacité requise : `Voulues` enrichit, le jeu reste jouable sans (REM-146, OUT-211). L'épreuve le joue avec toutes les capacités à `Non`.

**L'accord vérifié à la compilation.** `jeu::module!` évalue dans un `const` :

| Déclaration | Doit s'accorder avec |
|---|---|
| `telephones: SansObjet` | `GesteTelephone`, `TenuTelephone` et `VueTelephone` valent `Aucun`, et réciproquement |
| `secrets: Oui` | `Forme::SECRET` de l'état vaut `Oui`, et réciproquement |
| `prediction: Oui` | `telephones: Oui` |
| `manettes: SansObjet` | aucune contrainte de type ; une entrée `Manette` n'arrive jamais |

Un désaccord est une erreur de compilation. Ce qui ne se calcule pas en `const` (une phase terminale existe si `fin: Oui`) est vérifié par le moteur au chargement, qui refuse le module.

### 2.13 Les traits

```rust
pub trait Jeu: Etat + Sized + 'static {
    /// Les commandes de l'écran de l'ordinateur de la soirée. Un téléphone ne peut pas en envoyer :
    /// le décodeur de son canal ne connaît que `GesteTelephone` (SQ-21, SQ-22, PUN-48).
    type ActionEcran: Message;
    type GesteTelephone: Message;
    type TenuTelephone: Tenu;
    type VueTelephone: Message;
    /// Ce que dit un refus, au seul appareil du geste.
    type Avis: Message;
    type Phase: Phase;
    /// Ce qu'une nouvelle partie garde exprès de la précédente.
    type Garde: Etat;
    /// L'état de la version précédente du jeu, ou `Aucun`.
    type Precedent: Etat;

    const DECLARATION: Declaration;

    fn creer(c: Creation<'_, Self>) -> Self;
    fn appliquer(avant: Avant<'_, Self>, entree: Entree<'_, Self>) -> Issue<Self>;

    fn vue_ecran(&self, r: &Regard, s: &mut Scene<Self::ActionEcran>);
    fn vue_telephone(&self, r: &Regard) -> Montre<Self::VueTelephone>;
    /// Les octets d'une image citée par une scène, que le moteur n'a pas encore.
    fn ressource(&self, cle: Cle) -> Option<&[u8]>;

    fn phase(&self) -> Self::Phase;
    fn reveil(&self) -> Reveil;
    fn attente(&self, a: &mut Attente<'_, Self>);
    fn accueil(&self) -> Accueil;
    fn demandes<'s>(&'s self, d: &mut Demandes<'s>);

    /// Ce geste peut-il s'afficher avant confirmation ? Lu seulement si `prediction: Oui`.
    fn previsible(g: &Self::GesteTelephone) -> Prevision;
    fn garder(&self) -> Self::Garde;
    fn convertir(ancien: Self::Precedent) -> Result<Self, Raison>;
}

pub struct Creation<'c, J: Jeu> { /* privé */ }
impl<'c, J: Jeu> Creation<'c, J> {
    pub fn maintenant(&self) -> Maintenant<'_>;
    pub fn heure(&self) -> HeureMurale;
    pub fn autorite(&self) -> &Autorite;
    pub fn hasard(&mut self) -> Result<&mut Hasard, Absent>;
    /// Les appareils déjà là : vide au premier lancement, la salle entière après « On rejoue ! ».
    pub fn presents(&self) -> &[Presence];
    pub fn garde(&mut self) -> Option<J::Garde>;
}

/// Ce que le jeu donne à l'épreuve. `jeu::module!` ne compile pas sans.
pub trait Epreuve: Jeu {
    /// Ce qui résume l'histoire définitive : comparé entre deux exécutions qui doivent l'avoir égale,
    /// et passé à `transition`.
    type Resume: Message + PartialEq;
    /// Les écrans du jeu, pour la galerie des pires cas.
    type Cas: Variantes + Message + Copy + Eq;

    fn resume(&self, a: &Autorite) -> Self::Resume;
    fn invariants(&self, a: &Autorite, v: &mut Verif);
    fn transition(avant: &Self::Resume, apres: &Self::Resume, v: &mut Verif);
    /// Un téléphone simulé : il ne voit que ce que voit un humain qui tient ce téléphone
    /// et regarde la télé. Il ouvre les secrets avec `b.regard()`, jamais avec `Autorite`.
    fn bot_telephone(jeu: &Self, vue: &Montre<Self::VueTelephone>, b: &mut Bot<'_, Self>);
    /// L'organisateur simulé, à l'écran.
    fn bot_ecran(jeu: &Self, b: &mut Bot<'_, Self>);
    fn cas(&self) -> Self::Cas;
    /// Le pire cas de chaque écran, construit depuis les limites. Le `match` exhaustif les exige tous.
    fn pire_cas(cas: Self::Cas) -> Self;
    fn proprietes() -> Proprietes<Self>;
}

pub struct Verif { /* privé */ }
impl Verif { pub fn constater(&mut self, nom: &'static str, c: Constat); }
pub enum Constat { Tenu, Viole }
/// `jeu::exiger!(v, condition, "nom");`
#[macro_export] macro_rules! exiger { /* … */ }

pub struct Bot<'b, J: Jeu> { /* privé */ }
impl<J: Jeu> Bot<'_, J> {
    pub fn regard(&self) -> &Regard;
    pub fn style(&self) -> Style;
    pub fn hasard(&mut self) -> &mut Hasard;            // le sien, pas celui du jeu
    pub fn depuis_vue(&self) -> Duree;                  // depuis que sa vue a changé
    pub fn texte_hostile(&mut self) -> &'static str;    // corpus du commun : emoji, zalgo, contrôles, balisage, `{joueur}`…
    pub fn geste(&mut self, g: J::GesteTelephone);
    pub fn tenir(&mut self, t: J::TenuTelephone);
    pub fn action(&mut self, a: J::ActionEcran);
    pub fn touche(&mut self, t: Touche);
    pub fn composant(&mut self, c: ComposantBot);
    pub fn rien(&mut self);
}
pub enum Style { Sage, Hostile, Lent }
pub enum ComposantBot { Saisie { champ: Cle, texte: String, fin: FinSaisie }, Deplace(Deplace), Fichiers(Vec<FichierBot>) }
pub struct FichierBot { pub nom: String, pub genre: String, pub octets: Vec<u8> }

pub struct Proprietes<J: Jeu> {
    /// Une coupure plus courte que `moins_de`, sur un appareil pour lequel l'hypothèse tient
    /// à son début, ne change pas `resume` (PUN-73, SQ-28).
    pub coupure_transparente: Applicable<Coupure<J>>,
    /// Renuméroter les appareils ne change pas `resume`, ni les vues à renumérotation près (CAT-27).
    pub permutation: Applicable<()>,
}
pub struct Coupure<J> { pub moins_de: Duree, pub hypothese: fn(&J, &Autorite, Appareil) -> Oui }
```

### 2.14 Le module et la copie native

`jeu::module!(T)` écrit, sous `cfg(target_arch = "wasm32")` :

- l'allocateur global, à deux modes (§ 3.2), et le gestionnaire de panique, qui écrit le message dans la zone de panique puis exécute `unreachable` ;
- la boîte d'entrée, la zone de brouillon et la zone de panique, des statiques de taille fixe ;
- l'enveloppe : l'état du jeu, son `Secret<Hasard>`, le compteur d'identifiants, le pas courant, l'état des demandes connues ;
- les deux exports (§ 3.3) et la vérification `const` de la déclaration.

C'est le seul code `unsafe` d'un module, chacun avec `#[expect(unsafe_code, reason = "…")]`. Le reste de la colle (décoder, appeler le jeu, encoder) est du code sûr de `jeu::colle`, compilé à l'identique pour le module et pour la copie native.

**La copie native** (`jeu-natif`, § 8.4) compile la même crate de règles en natif, et appelle `jeu::colle` par une fonction Rust au lieu d'un export. Elle n'a pas les garanties de l'enfermement. L'épreuve compare, entrée par entrée, ses retours et sa forme canonique à ceux du module.

---

## 3. Le contrat binaire

### 3.1 La cible

- `wasm32-unknown-unknown`, version de Rust figée par `rust-toolchain.toml`, édition 2024, `panic = "abort"`, contrôle de débordement actif en release, LTO.
- **Aucun import.** Le moteur refuse un module qui en a un, en nommant l'import.
- **Pas de fonction de départ.**
- **Une mémoire**, exportée sous le nom `memory`, non partagée, 32 bits.
- **La pile en bas de la mémoire** (`--stack-first`) : un débordement de pile tombe hors de la mémoire et piège.
- **Fonctionnalités acceptées :** WebAssembly 1.0, globales mutables, extension de signe, conversions saturantes, valeurs multiples, mémoire en bloc (`memory.copy`, `memory.fill`, `memory.init`, `data.drop`), types référence pour `call_indirect` seulement.
- **Refusées :** fils, atomiques, mémoire partagée, SIMD (relâché ou non, dans cette version), mémoire 64 bits, plusieurs mémoires, exceptions, appels terminaux, GC, `table.set`, `table.grow`, `table.fill`, `table.copy`, `table.init`, `elem.drop`, et toute instruction hors de cette liste. Le refus nomme l'instruction et sa position.

### 3.2 La mémoire et ses zones

| Zone | Contenu | Dans l'empreinte de l'état |
|---|---|---|
| Pile | de 0 à la valeur initiale de `__stack_pointer` | non : rien ne la relit d'un appel à l'autre |
| Boîte | la boîte d'entrée statique, `capacite` octets | non |
| Brouillon | les allocations des lectures et toutes les sorties, `limites.brouillon` octets | non : remise à zéro au début de chaque appel |
| Panique | 4 Kio : un code et un message | non |
| Statiques et tas | `.data`, `.bss` (hors des trois zones ci-dessus), l'allocateur, tout ce que l'état alloue | oui |

- **L'allocateur a deux modes.** En mode tas, il sert `dlmalloc`. En mode brouillon, il sert une arène qui ne fait que monter. Il libère selon l'adresse : une adresse dans le brouillon ne touche pas le tas.
- **Les lectures sont en mode brouillon** de bout en bout. Une vue qui alloue ne touche pas l'état.
- **Les écritures sont en mode tas** pendant le décodage de tout ce qui peut entrer dans l'état, et pendant l'appel du jeu. L'enveloppe d'une écriture ne contient que des tranches empruntées et des nombres : ce qui entre dans l'état est décodé en mode tas, directement depuis les octets d'entrée. La sortie est encodée en mode brouillon.
- **L'empreinte de l'état** est le BLAKE3 de la mémoire hors des quatre zones exclues, de la taille de la mémoire et des globales mutables. Deux exécutions du même module sur le même journal ont la même empreinte, octet pour octet.
- **Entre deux appels**, le moteur vérifie que `__stack_pointer` a retrouvé sa valeur initiale et que `soiree_profondeur` vaut 0. Sinon : panne de l'outil.

### 3.3 Les exports

```wat
(module
  (memory (export "memory") $min $max)
  (func (export "soiree_boite") (param $taille i32) (param $sorte i32) (result i32))
  (func (export "soiree_appel") (param $taille i32) (result i64)))
```

- `soiree_boite(taille, sorte)` rend l'adresse où écrire `taille` octets d'entrée. `sorte` vaut 0 pour une lecture, 1 pour une écriture. Une lecture passe toujours par la boîte statique ; au-delà de sa capacité, la fonction rend 0 sans rien allouer. Une écriture au-delà de la capacité reçoit un tampon du tas, libéré à la fin de l'appel : le rejeu du journal refait la même allocation.
- `soiree_appel(taille)` lit un `Appel` à cette adresse, l'exécute, écrit un `Retour` dans le brouillon et rend `(adresse << 32) | longueur`.
- Aucun autre export n'est appelé. La réécriture ajoute ceux du moteur (§ 3.6).

### 3.4 Un appel

1. Le moteur encode l'`Appel`, et refuse au-dessus des limites avant d'écrire.
2. `a ← soiree_boite(n, sorte)`. `a = 0` : panne de l'outil.
3. Il écrit les `n` octets à `a`.
4. Il pose `soiree_carburant` au budget de cette sorte d'appel.
5. `r ← soiree_appel(n)`.
6. Il vérifie que `[r >> 32, (r >> 32) + (r & 0xffff_ffff))` est dans le brouillon, copie la sortie, et la décode strictement.
7. Un piège est classé par les globales et la zone de panique (§ 4.6).

| Sorte | Appels |
|---|---|
| Écriture | `Creer`, `Entree`, `Charger`, `DepuisCanonique`, `PireCas` |
| Lecture | tous les autres |

### 3.5 L'enveloppe

Les types du jeu y voyagent en octets. Le moteur ne les connaît pas : il les garde, les compare et les renvoie. Le module les décode lui-même, dans `jeu::colle`, avant d'appeler le code du jeu.

```rust
// jeu-abi : une définition, pour le moteur et pour la colle.
pub const ABI: u32 = 1;

pub enum Appel<'a> {
    /// Lecture. Le premier appel, juste après l'instanciation.
    Bonjour,
    /// Lecture. La déclaration.
    Decrire,
    /// Écriture. Crée l'état.
    Creer { graine: [u8; 32], pas: u64, heure: u64, presents: &'a [u8], garde: Option<&'a [u8]> },
    /// Lecture. Ces octets d'appareil sont-ils bien formés ? Rien de malformé n'entre au journal.
    Valider { sorte: SorteOctets, octets: &'a [u8] },
    /// Écriture. Une entrée.
    Entree { ordre: u64, place: u64, entree: &'a [u8] },
    /// Lecture. Les vues de ces regards.
    Regarder { regards: Vec<RegardFil> },
    /// Lecture. Phase, réveil, attente, accueil, demandes.
    Etat { connues: Vec<u64> },
    /// Lecture. Les octets des ressources citées.
    Ressources { cles: Vec<u64> },
    /// Lecture. La projection pour un regard, par morceaux.
    Projeter { regard: RegardFil, morceau: u32 },
    /// Écriture, sur un appareil qui prédit. L'état devient cette projection.
    Charger { projection: &'a [u8] },
    /// Lecture. La forme canonique, secrets compris, par morceaux.
    Canonique { morceau: u32 },
    /// Écriture. L'état depuis une forme canonique de cette forme, ou de la version précédente.
    DepuisCanonique { forme: u64, octets: &'a [u8] },
    /// Lecture. Ce qu'une nouvelle partie garde.
    Garder,
    /// Lecture, épreuve.
    Resumer,
    Verifier,
    Transition { avant: &'a [u8], apres: &'a [u8] },
    Bot { regard: RegardFil, style: StyleFil, graine: u64, depuis_vue: u64 },
    /// Écriture, épreuve. L'état devient ce pire cas.
    PireCas { indice: u32 },
    /// Lecture. Octets vivants du tas.
    Memoire,
}

pub enum SorteOctets { ActionEcran, GesteTelephone, TenuTelephone }

pub enum Retour<'a> {
    Bonjour(Bonjour),
    Decrit(DeclarationFil),
    Issue(IssueFil<'a>),
    Valide(Validation<'a>),
    Vues(Vec<VueFil<'a>>),
    Etat(EtatFil<'a>),
    Ressources(Vec<(u64, &'a [u8])>),
    Morceau { octets: &'a [u8], suivant: Option<u32> },
    Fait,
    Octets(&'a [u8]),                                       // garde, résumé
    Verif(Vec<(&'a str, ConstatFil)>),
    Bot(Vec<CoupFil<'a>>),
    PireCas { cas: &'a [u8] },
    Memoire { vivants: u64, tas: u64 },
    /// La frontière a refusé l'appel lui-même : panne de l'outil.
    Illisible(ErreurFil),
}

pub struct Bonjour {
    pub abi: u32,
    pub formes: u64,                 // empreinte des formes : enveloppe, état, et les types associés du jeu
    pub boite: (u32, u32),           // adresse, capacité
    pub brouillon: (u32, u32),
    pub panique: (u32, u32),
    pub version_jeu: (u16, u16, u16),// version de la crate `jeu` compilée dans le module
}

pub enum IssueFil<'a> {
    Acceptee { sorties: Vec<SortieFil>, reveil: ReveilFil },
    Refusee { avis: &'a [u8] },
    Ignoree,
    Attente,
}
pub enum SortieFil { Oublier(u64), Retirer(u64), NouvellePartie { graine: [u8; 32] } }
pub enum ReveilFil { ChaquePas, A(u64), Jamais }

pub enum Validation<'a> {
    Valide { tenu: Option<TenuFil<'a>> },
    Malforme(ErreurFil),
}
/// Ce que le moteur doit savoir d'un tenu sans connaître son type.
pub struct TenuFil<'a> { pub cle: &'a [u8], pub relache: Option<&'a [u8]>, pub est_relache: OuiFil }

pub struct VueFil<'a> { pub regard: RegardFil, pub octets: &'a [u8] }   // SceneFil ou MontreFil encodé
pub struct MontreFil<'a> { pub vue: &'a [u8], pub effets: Vec<Effet>, pub reglages: Reglages }

pub struct EtatFil<'a> {
    pub phase: &'a [u8],
    pub phase_indice: u16,           // dans `Phase::TOUTES`
    pub terminale: OuiFil,
    pub reveil: ReveilFil,
    pub attente: AttenteFil<'a>,
    pub accueil: Accueil,
    pub cas_indice: u16,
    pub demandes: Vec<u64>,          // toutes les identités listées
    pub nouvelles: Vec<(u64, DemandeFil<'a>)>,   // celles que `connues` n'avait pas
}
pub struct AttenteFil<'a> {
    pub echeance: Option<u64>,
    pub libre: Option<&'a str>,
    pub joueurs: Vec<(u64, Vec<&'a [u8]>)>,
    pub ecran: Vec<&'a [u8]>,
}

pub enum RegardFil { Ecran, Appareil(u64) }
pub enum StyleFil { Sage, Hostile, Lent }
pub enum ConstatFil { Tenu, Viole }
pub enum OuiFil { Oui, Non }
pub enum CoupFil<'a> { Geste(&'a [u8]), Tenu(&'a [u8]), Action(&'a [u8]), Touche(Touche), Composant(ComposantBotFil<'a>), Rien }
pub struct ErreurFil { pub ou: u32, pub quoi: &'static str }
```

`entree` dans `Appel::Entree` est l'encodage d'une `EntreeFil` :

```rust
pub enum EntreeFil<'a> {
    Temps { saut: u64, heure: u64 },
    Ecran { quoi: EcranFil<'a>, quand: QuandFil },
    Geste { de: u64, octets: &'a [u8], quand: QuandFil },
    Tenu { de: u64, octets: &'a [u8], cause: CauseTenu, quand: QuandFil },
    Manette { de: u64, etat: Manette, cause: CauseTenu, quand: QuandFil },
    Presence(Presence),
    Reponse { id: u64, reponse: ReponseFil<'a> },
}
pub struct QuandFil { pub fait: u64, pub fraction: u16, pub recu: u64, pub vu: u64, pub id: u64 }
pub enum EcranFil<'a> { Action(&'a [u8]), Touche(Touche), Pointeur(Pointeur), Composant(ComposantFil<'a>) }
```

`DeclarationFil`, `SceneFil`, `DemandeFil`, `ReponseFil`, `ComposantFil` sont les formes possédées ou empruntées des types de § 2, avec le même encodage postcard. Un test de `jeu-abi` encode chaque type de `jeu` et le décode comme son `Fil`, et réciproquement.

### 3.6 Ce que le moteur réécrit au chargement

La réécriture est une fonction pure des octets du module et de sa version. Son résultat a sa propre empreinte. Le module réécrit tourne pareil dans un navigateur et dans un moteur natif : budget, profondeur, NaN et croissance sont dans son code, pas dans le moteur qui l'exécute.

| Passe | Ce qu'elle fait | Ce qu'elle garantit |
|---|---|---|
| **Valider** | § 3.1 ; exactement les exports de § 3.3 ; maximum de mémoire présent | ce que le module peut faire se constate sur ses octets |
| **Carburant** | une globale `i64` `soiree_carburant`, décrémentée à l'entrée de chaque fonction et en tête de chaque boucle du nombre d'instructions du bloc ; `memory.copy` et `memory.fill` coûtent en plus leur longueur divisée par 64 ; négative : `unreachable` | toujours rendre la main ; une faute de budget se rejoue à l'identique (SQ-59) |
| **Profondeur** | une globale `i32` `soiree_profondeur`, incrémentée à l'entrée de chaque fonction, décrémentée à chaque sortie ; au-delà du plafond : `unreachable` | un dépassement de pile au même endroit sur tous les moteurs |
| **NaN** | après chaque opération flottante qui peut produire un NaN, un NaN devient le NaN canonique (`0x7fc0_0000`, `0x7ff8_0000_0000_0000`) | la seule source de non-déterminisme qui reste sans fils ni SIMD (SQ-68) |
| **Croissance** | chaque `memory.grow` vérifie `memory.size + delta <= soiree_memoire_max`, sinon rend -1 ; un échec du vrai `memory.grow` sous ce plafond pose `soiree_croissance_refusee` | la mémoire bornée est une faute du jeu ; une machine à court de mémoire est une panne de la machine |
| **Globales** | exporte chaque globale mutable d'origine sous `soiree_globale_<n>` | copier et restaurer l'état entier |

Les exports ajoutés : `soiree_carburant`, `soiree_profondeur`, `soiree_memoire_max`, `soiree_croissance_refusee`, `soiree_globale_<n>`. Le constat de la réécriture liste les imports (vide), les exports, la mémoire minimale et maximale, les familles d'instructions présentes et leur nombre : c'est le constat que demande le contrat.

### 3.7 Le format

- **postcard 1.x** des deux côtés, avec les mêmes types Rust. Entiers en varint, variants numérotés dans l'ordre, longueurs préfixées, aucune étiquette.
- **Décodage strict.** Un octet restant est un refus (REM-59, OUT-146, SAL-118). Une longueur plus grande que ce qui reste échoue avant toute réservation (REM-58).
- **Les flottants** ne voyagent que dans `Fini`, `Axe`, `Axe2` et ceux de l'état canonique. Une vue n'en a pas : ses longueurs sont des `Fixe`.
- **Une seconde lecture indépendante** du format, écrite sans `jeu-abi`, décode des journaux réels dans les tests (SAL-88, SAL-90).

### 3.8 Les versions

| Identité | D'où elle vient | Ce qu'elle protège |
|---|---|---|
| `ABI` | une constante de `jeu-abi`, montée à chaque changement de l'enveloppe | moteur et module parlent la même enveloppe ; hors de la plage du moteur, refus au chargement avec les deux numéros |
| `formes` | calculée par `Forme` depuis les types | une page, une projection ou une forme canonique d'une autre forme est refusée au lieu d'être lue de travers (SAL-41, PUN-44, REM-77) |
| empreinte du module | BLAKE3 des octets livrés, avant réécriture | la version du jeu : elle nomme le journal, les points de reprise et les rapports (OUT-37) |
| empreinte de la réécriture | `reecriture::VERSION` et BLAKE3 du résultat | deux moteurs comparent le même module réécrit |

Un instantané de mémoire ne vaut que pour la même empreinte de module. D'une version à l'autre, l'état passe par la forme canonique : même `formes`, il se recharge ; la forme de `Precedent`, il passe par `convertir` ; sinon, refus net, et les octets sont gardés sans être écrasés (HIS-51, TL-13, TL-67).

---

## 4. La partie qui fait foi

Ce comportement est celui de la crate `partie` (§ 8.6). Il ne dépend ni du navigateur, ni de wasmtime.

### 4.1 L'horloge

- Le numéro de pas est `⌊temps de partie / (1/60 s)⌋`. Le temps de partie suit l'horloge monotone de l'ordinateur de la soirée, sauf pendant une pause du commun.
- Après un retard, le moteur livre au plus `rattrapage` pas d'affilée, puis le numéro saute, et le premier `Temps` après le saut porte la durée jetée (`saut`). Une boucle ne s'emballe jamais (CAT-15, SQ-62).
- Le moteur ne livre `Temps` que si `reveil()` le demande : à chaque pas, une fois à l'échéance (ou au premier pas après elle), ou jamais. Un jeu au tour par tour ne coûte rien entre deux échéances.
- `Pause::DuCommun` : plus de `Temps`, et chaque tenu non relâché reçoit son relâchement, cause `Pause`.
- L'horloge monotone peut ne pas compter le sommeil (SAL-51) : la coque la recoupe avec l'heure murale et note tout écart au journal. Ce n'est pas l'affaire du cœur, qui reçoit le pas cible.

### 4.2 Deux instances

| Instance | Sa mémoire | Elle sert à |
|---|---|---|
| **Définitive** | l'état qui fait foi, jusqu'au dernier pas définitif | le journal, les demandes, les sorties, les vues des téléphones, les sous-arbres `definitif`, les effets `Definitive` |
| **Tête** | une copie de la définitive, plus la fenêtre rejouée | la scène de l'écran, les effets `Provisoire` |

Sans fenêtre (`fenetre: SansObjet`), la tête est la définitive : une seule instance, aucune copie, aucun rejeu. C'est le cas de Punchline et de Tier list.

### 4.3 Placer et ordonner

À l'arrivée d'un geste :

```
fait  ← clamp(fait annoncé, recu − demi-aller-retour mesuré − antidatage_max, recu)
place ← max(fait.pas, définitive.pas + 1)
vu    ← min(vu annoncé, pas de la dernière vue produite pour cet appareil)
```

- Un appareil ne peut pas antidater au-delà de sa demi-latence mesurée plus la marge déclarée (SAL-129). La latence se mesure par écho depuis l'ordinateur de la soirée : un appareil peut la grossir, pas la réduire.
- Dans un pas, les entrées passent dans l'ordre de `fait`. À instant égal, l'ordre est tiré d'un hachage de la graine d'ordre, du pas et de l'identité interne de l'appareil, jamais de l'`Appareil` exposé (CAT-27).
- Puis vient `Temps`, si le réveil le demande.
- Un jeu qui veut une résolution simultanée (deux coups du même pas, sur l'état d'avant) rassemble les gestes dans son état et les résout à `Temps` (CAT-28).

**Les tenus.** Le cœur tient, par appareil et par clé, la dernière valeur reçue, son numéro et son instant :

- il écarte un numéro plus petit que le dernier livré (REM-14) ;
- il ne livre qu'un changement de la valeur effective, au plus `debit_max` par seconde, le dernier gagnant (REM-13, PUN-03) ;
- il écarte un état fait plus tôt que `age_max`, sauf un relâchement, qui passe toujours (SQ-02, CAT-25) ;
- il livre le relâchement, cause `Silence`, quand `fraicheur` passe sans nouvelle de l'appareil (SQ-01) ; cause `Depart` au départ de l'appareil.

Ce que le cœur synthétise (un relâchement, un `Temps`) est écrit au journal comme une entrée ordinaire. Le journal suffit au rejeu, sans horloge.

### 4.4 La boucle

```
avancer(cible) :
  cible ← min(cible, tête.pas + rattrapage)              (saut noté sinon)
  tant que définitive.pas < cible − fenêtre :
      s ← définitive.pas + 1
      jouer s sur la définitive ; écrire s au journal
      exécuter les sorties de s ; lire l'état ; lancer et annuler les demandes
  si un geste est arrivé à une place ≤ tête.pas :
      tête ← instantané de la définitive ; rejouer de définitive.pas + 1 à tête.pas
  jouer la tête jusqu'à cible

regarder(regards) :
  téléphones : vues de la définitive
  écran : scène de la tête, chaque `definitif` remplacé par le nœud de même clé de la définitive
          (absent : vide) ; effets `Provisoire` de la tête, `Definitive` de la définitive
```

- La fusion se fait par clé, jamais par position : la tête peut être aux Résultats quand la définitive est encore à la Question, et le panneau des résultats reste vide au lieu d'être mal apparié (CAT-41).
- Le commun d'un appareil garde les identités d'effet déjà jouées, jusqu'à leur péremption. Un réaffichage, une reconnexion, un rejeu de la tête ou du journal revoit la même identité : rien ne rejoue (PUN-66, SQ-79, SAL-167). Un effet `Provisoire` que la tête ne montre plus avant que son pas soit définitif est annulé s'il n'a pas encore été joué.
- Une sortie (`Oublier`, `Retirer`, `NouvellePartie`) ne s'exécute qu'au pas définitif.
- `NouvellePartie` : `Garder` sur l'ancienne instance, instance neuve, `Creer` avec la graine de la sortie, la garde et les présents. L'ancienne mémoire disparaît. Une soirée ne garde que ce que le jeu garde exprès (CAT-73, CAT-75, PUN-15).

### 4.5 Le journal et les points de reprise

- Le journal est la suite, par partie, des entrées appliquées à la définitive, avec leur place, leur ordre et leur issue : `Acceptee` (avec ses sorties), `Refusee`, `Ignoree`, `Ecartee { faute }`. Il garde les refus : leur rejeu refait les mêmes allocations, et la mémoire reste identique.
- Toutes les 256 entrées, il note l'empreinte de l'état (SAL-158).
- Un point de reprise est un instantané de la définitive (mémoire, globales, taille), avec l'état du cœur : présences, tenus, compteurs, demandes en cours, identités d'effet jouées, graines. Il est pris à la création, à chaque nouvelle partie, et à intervalle (point de départ : 10 s de partie).
- Reprendre : instance neuve de la même empreinte de module, restaurer le point, rejouer le journal qui le suit en sautant les entrées écartées, vérifier chaque empreinte notée. Un écart est une panne du commun, avec la première entrée qui diverge.

### 4.6 Une faute

Une faute est : carburant épuisé, profondeur dépassée, panique, accès hors mémoire, mémoire au-delà du maximum déclaré, sortie hors du brouillon, sortie indécodable, `Attente` sur la définitive.

| Où | Ce que fait le cœur |
|---|---|
| Une écriture sur la définitive | restaurer le dernier point ; rejouer le journal jusqu'à l'entrée ; l'écrire `Ecartee { faute }` ; continuer sans elle. Le geste reçoit l'avis du commun « Ce geste n'a pas pu être pris en compte. » |
| Une écriture sur la tête | reconstruire la tête depuis la définitive, sans cette entrée ; la définitive la jugera à son tour |
| Une lecture | garder la dernière vue valide de chaque regard ; vérifier l'empreinte de l'état ; si elle a changé, restaurer comme une écriture |
| Trois `Temps` écartés de suite | ne plus livrer de temps ; l'écran dit « Le jeu est en panne » et propose de reprendre au dernier point sain sans les entrées suivantes, ou une nouvelle partie. Aucun appareil ne perd son lien, et le journal est gardé |

Chaque faute produit un **rapport** : le module, la graine, le point, les entrées jusqu'à la faute, l'entrée isolée, le carburant consommé et le message de panique. Il suffit à refaire la faute seule sur la machine du développeur (HIS-06, HIS-07, OUT-141).

Un second garde-fou tourne en temps réel : un appel qui dépasse une durée murale (bug du moteur, carburant mal compté) arrête l'instance. C'est une **panne de l'outil**, jamais une faute du jeu (HIS-06, OUT-11).

| Issue | Exemple | Qui a échoué |
|---|---|---|
| Faute du jeu | panique, budget, profondeur, mémoire, secret absent sur la définitive, vue invalide | le jeu |
| Panne de l'outil | `Retour::Illisible`, pile non rendue, garde-fou mural, empreinte de rejeu qui diverge | le commun |
| Panne de la machine | `soiree_croissance_refusee`, instanciation impossible | la machine |
| Refus | `Refusee`, `Ignoree` | personne : c'est une règle |

---

## 5. Secrets, projection et prédiction

### 5.1 La projection

`Projeter { regard }` encode la forme canonique de l'état, chaque secret que ce regard ne voit pas vidé. `Charger` la décode comme un état ordinaire, dont les secrets absents sont vides. La projection se calcule, elle ne s'écrit pas. Le hasard est un secret de l'enveloppe : il n'est jamais projeté, et un geste dont l'effet dépend d'un tirage ne se prédit pas.

### 5.2 Ce que reçoit chaque appareil

| L'appareil | Il reçoit | Il calcule |
|---|---|---|
| ne prédit pas | sa `Montre`, calculée sur la définitive avec son regard | rien |
| prédit | sa projection de la définitive, et le module | sa vue, depuis sa prédiction |

Les deux chemins donnent la même vue : une vue ne lit les secrets qu'à travers son regard, donc `vue(état, r) = vue(projection(état, r), r)`. L'épreuve le vérifie à chaque pas (§ 7.2). Une vue ou une projection est un état : la plus récente remplace l'autre, renvoyée en entier à l'arrivée et à chaque reconnexion, sans delta (SAL-28, SAL-112, SAL-113). Une vue est celle d'un pas, jamais d'un instant d'arrivée (SQ-86).

### 5.3 Prédire, c'est exécuter la même étape (phase 2)

Sur un appareil qui prédit, à chaque projection reçue au pas `d` :

```
Charger(projection)
pour s de d + 1 au pas estimé :
    ses propres gestes prévisibles placés à s, pas encore inclus
    son propre tenu à s
    Temps(s), si le réveil le demande
```

- Seuls les gestes de cet appareil sont ajoutés, et seulement ceux que `previsible` accepte.
- Un secret absent donne `attendre` : ce geste et les suivants attendent la confirmation.
- La projection dit, pour cet appareil, le dernier événement et le dernier tenu qu'elle inclut.
- Une correction se lisse, nœud par nœud, sur la durée que la vue déclare (`lisse`).
- Un `Definitif<T>` ne montre sa valeur que si elle vient d'une projection confirmée.
- La prédiction peut toujours être coupée : un appareil lent, suspendu, ou dont l'instance a fauté, montre la projection seule. La reprise est la même.
- La partie qui fait foi ne reçoit que des gestes, jamais des prédictions.

### 5.4 Nommer ce qu'un geste crée

Ce qu'un geste crée (un tir, une balle, un trait) se nomme depuis `quand.id`, jamais depuis un compteur de l'état. La prédiction et la partie qui fait foi le nomment pareil, et la clé de dessin et l'identité d'effet en découlent : une balle prédite et sa confirmation sont un seul nœud, un son de tir un seul effet.

---

## 6. Vues, page et rendu : ce qui reste au commun

Le noyau ne porte aucune de ces choses ; elles se construisent au-dessus, sans que le jeu change. Elles sont hors de la phase 1, sauf la validation des vues.

### 6.1 Valider ce qui s'affiche (phase 1)

Le moteur décode chaque vue comme une entrée venue de l'extérieur : clés uniques parmi les frères, profondeur et nombre de nœuds bornés, éléments dans le repère et sa marge, polices et images connues, taille sous `limites.vue`, effets sous `limites.effets`, effets `Definitive` seulement hors de la tête ou dans un sous-arbre `definitif`. Une vue invalide est une faute du jeu : la précédente reste.

### 6.2 Glisser entre deux pas

La télé dessine à un instant légèrement passé, entre deux scènes de la tête. Un nœud `glisse` glisse si la même clé existe aux deux pas et qu'aucun ne porte `rupture`. L'instant de dessin suit l'horloge des arrivées de scènes, avec une marge au-delà d'un pas (SQ-56, SQ-58). Le commun compte ses présentations réussies côté GPU, pas les tours de `requestAnimationFrame` (OUT-33).

### 6.3 Les composants

Les champs restent de vrais éléments du navigateur : contenu, curseur, sélection et composition gardés tant que leur clé existe (TL-33, PUN-40, HIS-47). Pendant une composition, aucune touche n'est un raccourci ni une validation (TL-32). Un raccourci cède au champ qui a le focus (TL-34). Le glisser, la copie qui suit le pointeur, la place visée dans l'ordre de l'état, le défilement près du bord en vitesse par seconde avec plafond au réveil, l'annulation : au composant `Deplacables` (TL-36 à TL-47). Le jeu reçoit `Survol`, `Depose`, `Annule`, en termes de son état.

### 6.4 Le son et l'écran

Le son ne joue qu'avec un contexte audio vraiment `running` ; avant le premier geste, l'effet est compté perdu, et le diagnostic le dit (SAL-174, SAL-175). Le son se programme à `quand`, corrigé du retard mesuré de la sortie (SAL-176). La touche `F8` pose une marque au journal (SAL-162, OUT-141) ; `diagnostic` ouvre le panneau de diagnostic.

### 6.5 Le compte à rebours

Il se calcule à chaque image depuis l'échéance de la vue et le pas estimé, qui ne recule jamais, même après une reconnexion (PUN-39, SQ-36, CAT-99). Aucune page ne compte seule.

### 6.6 La page téléphone

Chaque jeu code sa page dans `<jeu>-page`, avec la crate `jeu-page` du commun : elle reçoit `Montre<VueTelephone>` et le pas de la vue, et envoie par `page::geste(g) -> Result<(), Envoi>` et `page::tenir(t)`. `Envoi` dit `TropGros` ou `FilePleine` : jamais une perte silencieuse (SAL-07, OUT-197). Elle ne détient pas l'état, ne prédit rien et ne gère pas la connexion. Ses écouteurs, minuteurs et boucles appartiennent à une vue et s'arrêtent avec elle (REM-40, SAL-163) ; un geste en cours finit quand la vue change (REM-23). La page relâche tout au départ du premier plan, jamais sur une coupure du réseau (REM-10, REM-11). Le contrat de cette crate est l'objet de la phase 2.

---

## 7. L'épreuve

### 7.1 Ce qu'elle joue

Une soirée simulée, à vitesse machine, sans navigateur ni réseau réels (CAT-112, OUT-83) : la `Partie` du cœur, sur une horloge simulée, avec autant de téléphones simulés que `places` le permet et un organisateur simulé. Les bots du jeu jouent depuis leur vue. Le transport simulé livre ce que le commun livrerait : chaque événement une fois, dans l'ordre par appareil ; les tenus par rafales, le dernier gagnant ; avec latence, gigue et rafales de Wi-Fi.

Les pannes sont choisies, mêlées et constatées avant d'être jugées (OUT-26, OUT-77, OUT-79, SAL-180) : coupure courte et longue, silence sans fermeture, trou noir, paquet retenu puis livré, rechargement d'un téléphone, onglet jumeau, redémarrage de l'autorité depuis son point et son journal, gel de 0 à 10 minutes.

### 7.2 Les contrôles

| # | Contrôle | Ce qu'il fait | Ce qu'il lit du jeu |
|---|---|---|---|
| E1 | Constat | réécriture acceptée, aucun import, déclaration cohérente, budgets d'appel mesurés | le module |
| E2 | Parcours | les bots vont jusqu'à une phase terminale, puis `NouvellePartie`, puis une autre fin (OUT-29) | `phase`, `fin` |
| E3 | Couverture | chaque variante de `Phase` et de `Cas` atteinte ; sinon le rapport nomme celle qui manque | `phase`, `cas` |
| E4 | Invariants | à chaque pas définitif ; `transition` à chaque entrée acceptée ; un invariant jamais évalué n'a pas réussi (OUT-01, HIS-01) | `Epreuve` |
| E5 | Jamais figée | après chaque pas définitif : chaque attendu présent a un exemple accepté (essayé sur une copie), ou une échéance existe ; un attendu absent sans échéance est une faute (CAT-78) | `attente` |
| E6 | Refus sans trace | empreinte de la forme canonique avant et après chaque refus et chaque `ignorer` | rien |
| E7 | Lecture sans trace | empreinte des zones d'état avant et après chaque lecture | rien |
| E8 | Secrets | `vue(état, r) = vue(projection(état, r), r)` pour chaque regard, à chaque pas ; les octets de chaque valeur secrète non montrée cherchés dans les vues des autres regards (CAT-46) | rien |
| E9 | Déterminisme | chaque soirée deux fois sur deux instances du module : empreinte après chaque entrée ; puis la copie native : retours et forme canonique | rien |
| E10 | Permutation | même soirée, autre tirage des `Appareil` : `resume` égal, vues égales à renumérotation près (CAT-27) | `proprietes` |
| E11 | Coupure transparente | même soirée avec et sans coupures plus courtes que `moins_de`, là où l'hypothèse tient : `resume` égal (PUN-73) | `proprietes` |
| E12 | Fenêtre | même soirée, gestes à l'heure ou en retard dans la fenêtre : même journal définitif | `fenetre` |
| E13 | Entrées absurdes | octets au hasard dans chaque champ de l'enveloppe et chaque `Valider` : `Malforme` ou `Illisible`, jamais une faute ; bots `Hostile` | rien |
| E14 | Reprise | arrêter l'autorité à un pas tiré, reprendre depuis le point et le journal : même empreinte | rien |
| E15 | Mémoire | six heures simulées : pente des octets vivants et du nombre d'appareils retenus pour les présents (OUT-84, HIS-10, SQ-76, PUN-15) | `Memoire` |
| E16 | Pires cas | chaque `Cas` : `cas(pire_cas(c)) = c`, invariants tenus, scène valide (la mesure des débordements vient avec le rendu) | `pire_cas` |
| E17 | Flottants | aucun flottant non fini dans la forme canonique ; aucun `Fixe` saturé | rien |
| E18 | Gel | après un gel, au plus une transition de phase par entrée, et une phase complète après la reprise | rien |
| E19 | Budgets | carburant par sorte d'appel, en distribution ; le 99ᵉ centile sous le budget, avec `(fenêtre + 2) × pas` sous une image | rien |

Chaque contrôle a un **contrôle négatif** : un jeu fautif de `exemples/fautifs` qui doit le faire échouer (OUT-27, HIS-03, SAL-168). Le rapport distingue réussi, échec du jeu (avec graine et journal pour le rejouer), panne de l'outil, sans objet (avec la raison déclarée) et pas exécuté (OUT-01, OUT-02, HIS-05, HIS-06). Une attente qui expire dit sa dernière valeur, sa durée et le nombre de pas vus (OUT-94). Les mutations du code du jeu et l'examen des leçons viennent ensuite, dans l'outil.

---

## 8. Le découpage du commun pour la phase 1

**Le but de la phase 1 :** un jeu écrit avec `jeu`, compilé en module, réécrit, chargé dans wasmtime, joué par le cœur sur une horloge simulée, et éprouvé, sans navigateur. Le relais, la page, le rendu et la prédiction sur appareil viennent en phase 2.

```
soiree/crates/
├── jeu-abi/            le vocabulaire du commun et l'enveloppe            no_std
├── jeu-derive/         #[jeu::message], #[jeu::etat], #[derive(Variantes)]
├── jeu/                ce qu'écrit l'auteur, la colle, jeu::module!       no_std
├── jeu-natif/          la copie native d'un jeu, comme `Instance`
├── reecriture/         valider et réécrire un module                      pure
├── moteur-wasmtime/    le module réécrit dans wasmtime, comme `Instance`
├── partie/             le cœur de la partie qui fait foi                  sans entrées-sorties
├── epreuve/            la soirée simulée et ses contrôles, et `soiree-epreuve`
└── exemples/
    ├── devine/         le jeu d'exemple de § 9, avec sa crate de module
    └── fautifs/        un jeu par contrôle négatif
```

Les dépendances tierces, chacune avec sa raison à côté d'elle : `serde` et `postcard` (le format, figé), `libm` (des flottants identiques partout), `unicode-normalization` et `unicode-segmentation` (le texte), `dlmalloc` (un allocateur mûr, celui de `std` sur cette cible), `syn`, `quote`, `proc-macro2` (les macros), `wasmparser` et `wasm-encoder` (lire et écrire un module), `wasmtime` (le moteur natif), `blake3` (les empreintes, côté moteur), `proptest` (les tests).

Le graphe : `jeu-abi` ← `jeu` ← `jeu-natif` ; `jeu-abi` ← `partie` ← `moteur-wasmtime`, `jeu-natif`, `epreuve` ; `reecriture` ← `moteur-wasmtime`. Le moteur et le cœur ne dépendent jamais de `jeu` : ils ne connaissent aucun type de jeu.

### 8.1 `jeu-abi`

**Rôle.** La définition unique de ce que le commun et un module échangent : l'enveloppe (§ 3.5), le vocabulaire du commun (temps, appareils, capacités, entrées de l'écran, manette, effets, scène, demandes, réponses, déclaration en forme possédée), les valeurs validées (`Fini`, `Axe`, `Axe2`, `Borne`, `Cumul`, `Bouton`, `Fixe`, `TexteCourt`, `CodeTouche`), `Cle::de` et `IdEffet::de` (FNV-1a 64).

**API publique.** Les types de § 2.3 à § 2.12 qui n'ont pas de paramètre de jeu, et ceux de § 3.5. Plus :

```rust
pub const ABI: u32;
pub fn encoder<T: serde::Serialize>(v: &T, dans: &mut alloc::vec::Vec<u8>) -> Result<(), ErreurEncodage>;
pub fn decoder<'a, T: serde::Deserialize<'a>>(octets: &'a [u8]) -> Result<T, ErreurDecodage>;   // strict
impl Appel<'_> { pub fn sorte(&self) -> SorteAppel; }                                          // Lecture | Ecriture
pub fn fnv1a64(octets: &[u8]) -> u64;
```

**Tests attendus.**
- `decodage_refuse_un_octet_restant`, `longueur_absurde_echoue_sans_reserver` (REM-58, REM-59).
- `fini_refuse_nan_infini_et_ramene_les_sous_normaux`, `axe2_refuse_une_norme_au_dela_de_un`, `borne_refuse_hors_plage`.
- `chaque_appel_a_sa_sorte` : la table de § 3.4, écrite à part, contre `sorte()`.
- `forme_figee` : l'encodage de chaque variant de `Appel`, `Retour` et `EntreeFil` comparé à des octets écrits à la main dans le test ; un changement sans montée d'`ABI` rougit (PUN-90).
- `seconde_lecture` : un décodeur écrit sans `jeu-abi`, dans le test, lit les mêmes octets (SAL-90).
- Fuzz : `decoder::<Appel>` et `decoder::<EntreeFil>` sur des octets au hasard ne paniquent jamais.

### 8.2 `jeu-derive`

**Rôle.** Les trois macros de § 2.2. Aucune logique de jeu.

**API publique.** `#[jeu::message]`, `#[jeu::etat]`, `#[derive(Variantes)]`, réexportées par `jeu`.

**Tests attendus.** Avec `trybuild` :
- compilent : struct, enum, génériques bornés, champs `Secret`, `Partage`, `Appareils` dans un état ;
- ne compilent pas : `Secret` dans un `#[jeu::message]` ; `Cell`, `RefCell`, `HashMap`, `Rc` dans un état ; chaque attribut `serde` refusé ; `Variantes` sur un variant à champs.
- `empreinte_change_avec_un_champ`, `empreinte_change_avec_l_ordre_des_variants`, `empreinte_stable_entre_deux_compilations`.
- `projection_vide_ce_que_le_regard_ne_voit_pas`, pour chaque `Qui`.

### 8.3 `jeu`

**Rôle.** Tout ce que l'auteur écrit (§ 2), et la colle du module : décoder, appeler le jeu, encoder, l'enveloppe, l'allocateur à deux modes, `jeu::module!`.

**API publique.** § 2 en entier, plus `jeu::prelude`, `jeu::math` et, cachés de la documentation, `jeu::colle::{Enveloppe, appeler}` et `jeu::cache`.

```rust
#[doc(hidden)]
pub mod colle {
    /// La partie entière telle que la tient le module.
    pub struct Enveloppe<J: Jeu + Epreuve> { /* J, Secret<Hasard>, compteur d'ids, pas, demandes connues */ }
    /// Le même chemin pour le module et pour la copie native.
    pub fn appeler<J: Jeu + Epreuve>(e: &mut Option<Enveloppe<J>>, entree: &[u8], sortie: &mut Sortie) -> Result<(), ErreurFil>;
}
```

**Tests attendus.**
- `refus_ne_change_rien` : propriété sur `exemples/devine`, toute suite d'entrées ; forme canonique avant et après chaque refus égale.
- `lecture_n_alloue_pas_dans_le_tas` : compte des allocations du tas pendant chaque lecture, nul.
- `ecriture_decode_en_mode_tas` : une entrée dont le texte entre dans l'état reste lisible après la remise à zéro du brouillon.
- `texte_nettoyer_est_idempotent`, `texte_coupe_entre_graphemes`, `texte_capitales_compte_eszett_double`, `texte_cle_ignore_selecteurs_et_liants`, `texte_decode_refuse_un_texte_sale` (PUN-22 à PUN-26, CAT-80, CAT-82, CAT-83).
- `hasard_vecteurs_de_reference` (xoshiro256**), `borne_sans_biais` et `melanger_uniforme` à graines fixées (CAT-89 à CAT-91, CAT-118).
- `echeance_ne_nait_que_de_maintenant` (`trybuild`) ; `maintenant_ne_se_garde_pas` (`trybuild`).
- `accord_declaration_types` (`trybuild`) : chaque ligne de la table de § 2.12 dans les deux sens.
- `raison_creuse_refusee` (`trybuild`) : `raison!("")`, `raison!("n/a")`.
- `forme_canonique_aller_retour` sur des états tirés au hasard ; `partage_ecrit_une_fois`.
- `colle_natif_et_module_identiques` : vit dans `epreuve` (E9), cité ici parce qu'il garde la colle.

### 8.4 `jeu-natif`

**Rôle.** La copie native d'un jeu comme `partie::Instance`, pour les tests rapides et la comparaison avec le module.

**API publique.**

```rust
pub struct InstanceNative<J: Jeu + Epreuve> { /* Option<Enveloppe<J>> */ }
pub struct FabriqueNative<J>(PhantomData<J>);
impl<J: Jeu + Epreuve> partie::Fabrique for FabriqueNative<J> { type I = InstanceNative<J>; /* … */ }
impl<J: Jeu + Epreuve> partie::Instance for InstanceNative<J> { /* … */ }
```

- `appeler` passe par `jeu::colle::appeler`, sous `catch_unwind` : une panique devient `Faute::Panique`. Sans carburant : la copie native n'a pas de budget.
- `instantane` clone l'enveloppe ; `restaurer` la remplace ; `empreinte` est le BLAKE3 de la forme canonique.

**Tests attendus.** `panique_devient_faute`, `instantane_puis_restaurer_rend_la_meme_empreinte`, `meme_journal_meme_empreinte`.

### 8.5 `reecriture`

**Rôle.** Valider un module et le réécrire (§ 3.6). Une fonction pure, sans wasmtime, réutilisée telle quelle par le moteur du navigateur en phase 2.

**API publique.**

```rust
pub const VERSION: u32 = 1;
pub struct Reglages { pub profondeur_max: u32, pub memoire_plafond_pages: u32 }
pub fn reecrire(original: &[u8], r: &Reglages) -> Result<Reecrit, Refus>;
pub struct Reecrit {
    pub octets: Vec<u8>,
    pub empreinte_originale: [u8; 32],
    pub empreinte: [u8; 32],
    pub constat: Constat,
    pub pile_initiale: u32,              // valeur initiale de __stack_pointer
}
pub struct Constat {
    pub imports: Vec<(String, String)>,  // toujours vide si accepté
    pub exports: Vec<String>,
    pub memoire: (u32, Option<u32>),
    pub familles: Vec<(&'static str, u32)>,
    pub globales_mutables: u32,
}
pub enum Refus {
    Illisible { position: usize, quoi: String },
    Import { module: String, nom: String },
    Export { nom: String },
    ExportManquant { nom: &'static str },
    Fonctionnalite { nom: &'static str, position: usize },
    Instruction { nom: &'static str, position: usize },
    Depart,
    Memoire(RefusMemoire),               // absente, plusieurs, partagée, 64 bits, sans maximum
    PileNonEnBas,
}
pub mod passes {
    pub fn valider(m: &[u8]) -> Result<Constat, Refus>;
    pub fn carburant(m: &[u8]) -> Result<Vec<u8>, Refus>;
    pub fn profondeur(m: &[u8], max: u32) -> Result<Vec<u8>, Refus>;
    pub fn nan(m: &[u8]) -> Result<Vec<u8>, Refus>;
    pub fn croissance(m: &[u8], plafond: u32) -> Result<Vec<u8>, Refus>;
    pub fn globales(m: &[u8]) -> Result<Vec<u8>, Refus>;
}
```

**Tests attendus.**
- Un refus par ligne de § 3.1, sur un petit module écrit en `wat` dans le test, qui nomme l'import ou l'instruction.
- **Chaque passe rougit quand on la retire** : une boucle infinie ne rend pas la main sans `carburant` ; une récursion profonde diverge entre deux réglages de pile sans `profondeur` ; un NaN calculé a des bits différents sous deux moteurs sans `nan` ; un `memory.grow` dépasse le plafond sans `croissance`.
- `reecrire_est_pure` : deux réécritures du même module donnent les mêmes octets.
- `reecrire_preserve_le_sens` : sur `exemples/devine` et un corpus de modules `wat`, avec un carburant très grand, mêmes sorties avant et après réécriture.
- `carburant_compte_pareil` : le carburant consommé par un appel est le même à chaque exécution.
- `reecrit_se_valide` : le module réécrit passe `wasmparser::validate`.

### 8.6 `partie`

**Rôle.** Le cœur de la partie qui fait foi (§ 4) : placer, ordonner, tenir les tenus, avancer la définitive et la tête, fusionner les vues, suivre les demandes, isoler une faute, tenir le journal, reprendre. Sans horloge, sans réseau, sans fil, sans `HashMap`, sans WebAssembly : il appelle une `Instance`.

**API publique.**

```rust
pub trait Instance {
    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec>;
    fn instantane(&self) -> Result<Instantane, PanneOutil>;
    /// Après restauration, la taille de la mémoire est celle de l'instantané : une instance plus
    /// grande est remplacée par une neuve.
    fn restaurer(&mut self, i: &Instantane) -> Result<(), PanneOutil>;
    fn empreinte(&self) -> Result<[u8; 32], PanneOutil>;
}
pub trait Fabrique { type I: Instance; fn neuve(&mut self) -> Result<Self::I, PanneOutil>; }
pub enum Echec { Faute(Faute), Panne(PanneOutil) }
pub enum Faute { Budget, Profondeur, Panique { message: String }, Memoire, HorsMemoire, SortieInvalide, Piege { code: String } }

pub struct Ouverture { pub graine: [u8; 32], pub pas: u64, pub heure: u64, pub budgets: Budgets, pub point_tous_les: u64 }
pub struct Budgets { pub entree: u64, pub lecture: u64, pub creation: u64, pub canonique: u64 }

pub struct Partie<F: Fabrique> { /* définitive, tête, journal, tenus, présences, demandes, effets, graines */ }
impl<F: Fabrique> Partie<F> {
    pub fn ouvrir(f: F, o: Ouverture) -> Result<Partie<F>, RefusOuverture>;
    pub fn declaration(&self) -> &DeclarationFil;
    /// Une arrivée du transport, déjà dédoublonnée par appareil. `a_recu` : son instant de réception.
    pub fn arriver(&mut self, a: Arrivee, a_recu: Instant) -> Placement;
    pub fn avancer(&mut self, cible: Pas) -> Avancee;
    pub fn regarder(&mut self, regards: &[RegardFil]) -> Vec<Vue>;
    pub fn etat(&mut self) -> Result<EtatVisible, Echec>;
    pub fn ressources(&mut self, cles: &[u64]) -> Vec<(u64, Vec<u8>)>;
    pub fn point(&mut self) -> Result<PointDeReprise, PanneOutil>;
    pub fn journal(&self) -> &Journal;
    pub fn reprendre(f: F, o: Ouverture, p: &PointDeReprise, j: &Journal) -> Result<Partie<F>, RefusReprise>;
}

pub enum Arrivee {
    Geste { de: Appareil, octets: Vec<u8>, fait: Instant, vu: Pas, id: IdGeste, demi_aller_retour: Duree },
    Tenu { de: Appareil, octets: Vec<u8>, numero: u64, fait: Instant, demi_aller_retour: Duree },
    Manette { de: Appareil, etat: Manette, numero: u64, fait: Instant },
    Ecran { quoi: EcranArrivee, vu: Pas },
    Presence(Presence),
    Reponse { id: IdDemande, reponse: ReponseArrivee },
}
pub enum Placement { Place { pas: Pas }, Malforme(ErreurFil), HorsScene, Perimee }
pub struct Avancee {
    pub definitif: Pas,
    pub avis: Vec<(Appareil, Vec<u8>)>,
    pub avis_ecran: Vec<Vec<u8>>,
    pub lancer: Vec<(IdDemande, DemandeArrivee)>,
    pub annuler: Vec<IdDemande>,
    pub oublier: Vec<Appareil>,
    pub retirer: Vec<Appareil>,
    pub fautes: Vec<RapportFaute>,
    pub saut: Duree,
    pub en_panne: Oui,
}
pub struct Vue { pub regard: RegardFil, pub pas: Pas, pub octets: Vec<u8> }
pub struct EtatVisible { pub phase_indice: u16, pub terminale: Oui, pub attente: AttenteFil, pub accueil: Accueil, pub cas_indice: u16 }

/// Les règles, en fonctions pures : c'est elles que la preuve visera d'abord.
pub mod regles {
    pub fn placer(fait: Instant, recu: Instant, demi_aller_retour: Duree, antidatage_max: Duree, definitif: Pas) -> (Instant, Pas);
    pub fn ordonner(entrees: &mut [Placee], graine_ordre: &[u8; 32]);
    pub fn tenu(etat: &mut EtatTenu, arrivee: ArriveeTenu, regles: &ReglesTenu, pas: Pas) -> Option<Livraison>;
    pub fn silences(etats: &mut [EtatTenu], regles: &ReglesTenu, pas: Pas) -> Vec<Livraison>;
    pub fn fusionner(tete: &SceneFil, definitive: &SceneFil) -> SceneFil;
    pub fn demandes(en_cours: &[u64], listees: &[u64]) -> (Vec<u64>, Vec<u64>);   // à lancer, à annuler
    pub fn cible(definitif: Pas, tete: Pas, voulu: Pas, rattrapage: u8) -> (Pas, Duree);
}
```

**Tests attendus.** Sur une `InstanceModele` (un jeu écrit directement contre l'enveloppe, en Rust, dans les tests), puis sur `jeu-natif` et `exemples/devine` :
- `refus_ecrit_au_journal_et_rejoue` ; `faute_isolee_ne_se_rejoue_pas` ; `trois_temps_fautifs_mettent_en_panne` ; `faute_en_lecture_garde_la_derniere_vue`.
- `reprise_rend_la_meme_empreinte` depuis chaque point, avec un journal de 10 000 entrées ; `divergence_de_rejeu_est_une_panne_de_l_outil`.
- `fenetre_meme_journal_definitif` : gestes à l'heure ou en retard dans la fenêtre (E12, en petit).
- `sans_fenetre_une_seule_instance` : aucun `instantane` n'est pris hors des points.
- `gel_rattrape_au_plus_puis_saute` (CAT-15, SQ-62) ; `meme_temps_autrement_decoupe_meme_partie` sous le plafond de rattrapage (CAT-14).
- `antidatage_borne` (SAL-129) ; `egalite_de_fait_ne_depend_pas_de_l_appareil` (CAT-27) ; `vu_borne_par_la_derniere_vue`.
- `tenu_numero_ancien_ecarte`, `tenu_dernier_gagne_au_debit`, `tenu_trop_vieux_ecarte_sauf_relache`, `tenu_silence_relache`, `tenu_depart_relache`, `pause_du_commun_relache_tout` (REM-13, REM-14, SQ-01, SQ-02, CAT-25).
- `reponse_d_une_demande_retiree_jetee`, `demande_lancee_une_fois_meme_apres_reprise` (TL-45, CAT-76).
- `sortie_executee_au_definitif_seulement` ; `nouvelle_partie_instance_neuve_memoire_rendue` (CAT-73, CAT-75).
- `fusion_par_cle_jamais_par_position` ; `effet_definitive_lu_sur_la_definitive` ; `effet_provisoire_retire_annule`.
- `action_ecran_hors_scene_refusee`, `saisie_au_dela_de_la_limite_refusee` (SAL-152).
- Propriétés (`proptest`) sur `regles` : `placer` ne rend jamais une place avant `definitif + 1` ni un `fait` après `recu` ; `ordonner` est une permutation stable par `fait` ; `demandes` est exacte ; `cible` ne dépasse jamais `tete + rattrapage`.

### 8.7 `moteur-wasmtime`

**Rôle.** Charger un module réécrit dans wasmtime et l'exposer comme `partie::Instance`.

**API publique.**

```rust
pub struct Moteur { /* wasmtime::Engine */ }
impl Moteur {
    pub fn new(r: ReglagesMoteur) -> Result<Moteur, PanneOutil>;
    pub fn charger(&self, original: &[u8]) -> Result<ModuleCharge, RefusModule>;
}
pub struct ReglagesMoteur { pub reecriture: reecriture::Reglages, pub garde_fou_murale_ms: u32 }
pub struct ModuleCharge { /* wasmtime::Module, Reecrit, Bonjour */ }
impl ModuleCharge {
    pub fn constat(&self) -> &reecriture::Constat;
    pub fn empreinte(&self) -> [u8; 32];
    pub fn fabrique(&self) -> FabriqueWasm;
}
pub enum RefusModule { Reecriture(reecriture::Refus), Abi { module: u32, moteur: (u32, u32) }, Declaration(String), Panne(PanneOutil) }
pub struct FabriqueWasm { /* … */ }
pub struct InstanceWasm { /* Store, Instance, Memory, globales, zones */ }
impl partie::Fabrique for FabriqueWasm { type I = InstanceWasm; /* … */ }
impl partie::Instance for InstanceWasm { /* … */ }
```

Réglages de wasmtime : Cranelift ; fils, SIMD, SIMD relâché, plusieurs mémoires, mémoire 64 bits, appels terminaux, exceptions et GC désactivés ; types référence, mémoire en bloc et valeurs multiples activés ; pas de carburant de wasmtime ni de NaN canoniques de wasmtime (la réécriture les porte, et ses tests doivent pouvoir rougir) ; interruption par époque pour le garde-fou mural ; pile native assez grande pour que `soiree_profondeur` pique toujours avant elle.

`charger` réécrit, compile, instancie une fois, appelle `Bonjour` et `Decrire`, vérifie l'ABI, les formes, les zones et l'accord de la déclaration (§ 2.12), puis pose `soiree_memoire_max` à `limites.memoire`.

**Tests attendus.**
- `module_avec_import_refuse`, `abi_inconnue_refusee`, `declaration_incoherente_refusee`.
- `boucle_infinie_faute_budget`, `recursion_faute_profondeur`, `panique_faute_avec_message`, `memoire_au_dela_faute_memoire`, `acces_hors_memoire_faute`.
- `garde_fou_mural_panne_de_l_outil` : un carburant très grand sur une boucle infinie.
- `instantane_restaurer_meme_empreinte`, `restaurer_dans_plus_grand_cree_une_instance_neuve`.
- `pile_non_rendue_panne_de_l_outil` (module `wat` qui ne remet pas `__stack_pointer`).
- `deux_instances_meme_journal_meme_empreinte` sur `exemples/devine`.

### 8.8 `epreuve`

**Rôle.** La soirée simulée et les contrôles de § 7, et le binaire `soiree-epreuve <module.wasm>`, qui écrit un rapport.

**API publique.**

```rust
pub struct Plan {
    pub soirees: u32,
    pub duree: Duree,
    pub graine: [u8; 32],
    pub pannes: Vec<Panne>,
    pub controles: Vec<IdControle>,        // E1 à E19 ; un périmètre réduit reste visible dans le rapport (OUT-10)
}
pub enum Panne { Coupure { duree: Duree }, Silence { duree: Duree }, TrouNoir { duree: Duree }, Retenu { duree: Duree }, Rafales, Rechargement, Jumeau, RedemarrageAutorite, Gel { duree: Duree } }
pub fn eprouver(module: &moteur_wasmtime::ModuleCharge, natif: Option<&mut dyn FabriqueDynamique>, plan: &Plan) -> Rapport;
pub struct Rapport { pub module: [u8; 32], pub plan: Plan, pub controles: Vec<ResultatControle> }
pub struct ResultatControle { pub id: IdControle, pub issue: IssueControle, pub evaluations: u64, pub detail: String, pub rejeu: Option<Rejeu> }
pub enum IssueControle { Reussi, EchecDuJeu, PanneDeLOutil, SansObjet(String), PasExecute }
pub struct Rejeu { pub graine: [u8; 32], pub journal: Journal, pub point: Option<PointDeReprise> }
pub fn rejouer(module: &moteur_wasmtime::ModuleCharge, r: &Rejeu) -> Result<RapportFaute, PanneOutil>;
```

**Tests attendus.**
- `devine_passe_tout` : `exemples/devine` réussit E1 à E19, ou les déclare sans objet avec sa raison.
- **Un contrôle négatif par contrôle** : chaque jeu de `exemples/fautifs` fait échouer exactement le contrôle qu'il vise, et le rapport le nomme. Au moins : `refuse_apres_avoir_ecrit` (un `static` atomique écrit avant un refus : E6), `vue_qui_fuit` (copie un secret dans un champ public trop tôt : E8), `vue_qui_ecrit` (un `Partage` oublié par `mem::forget` dans une vue : E7), `attend_un_absent` (E5), `jamais_fini` (E2), `depend_de_l_appareil` (départage par la valeur d'un `Appareil` transmise dans un message : E10), `coupure_qui_compte` (E11), `fuit_la_memoire` (E15), `panique_au_temps` (trois `Temps` fautifs), `boucle_au_vote` (E19), `nan_dans_l_etat` (E17), `natif_different` (une règle sous `cfg(target_arch)` : E9).
- `panne_injectee_constatee` : une panne déclarée qui n'a pas eu lieu rend le contrôle `PasExecute`, jamais `Reussi` (OUT-26).
- `zero_evaluation_n_est_pas_un_succes` (OUT-07).
- `rejeu_refait_la_faute` : le rapport d'un fautif, rejoué seul, redonne la même faute.

### 8.9 L'ordre de construction

1. `jeu-abi`, puis `reecriture` et `moteur-wasmtime` sur des modules `wat` : charger, appeler, couper, restaurer.
2. `jeu-derive` et `jeu` sans vue, puis `exemples/devine` et sa copie native.
3. `partie` sans fenêtre, sur `InstanceModele`, puis sur les deux instances.
4. `epreuve` : E1 à E9 et E13 à E15 d'abord, chacun avec son négatif ; puis le reste.
5. La fenêtre et la tête ; E12.
6. La scène et sa validation ; E16.

Chaque étape s'arrête sur des tests verts et des contrôles négatifs rouges.

---

## 9. Un jeu complet : `devine`

Le jeu d'exemple de la phase 1. Chaque téléphone propose un chiffre ; l'autorité en a tiré un, secret ; à l'échéance, la télé révèle. Il touche chaque partie de l'interface : secret, hasard, refus, échéance, tenu, présence, demande, vue, bots, invariants.

```rust
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;
use alloc::vec::Vec;
use jeu::prelude::*;

jeu::limite!(pub Prenom = 12 Capitales);
const ESSAIS: Duree = Duree::s(20);
const RESULTATS: Flux = Flux::new("resultats");
type Chiffre = Borne<0, 9>;

#[jeu::etat]
pub struct Devine {
    phase: Etape,
    phase_depuis: Pas,
    fin: Option<Echeance>,
    nombre: Secret<Chiffre>,                    // Qui::Personne, puis Qui::Tous à la révélation
    joueurs: Appareils<Joueur>,
    manche: u32,
    lignes: Vec<(IdDemande, Vec<u8>)>,          // une ligne de résultats par manche, jusqu'à sa réponse
}
#[jeu::etat]
pub struct Joueur { prenom: Texte<Prenom>, essai: Secret<Option<Chiffre>>, curseur: Chiffre, points: u32, present: Oui }

#[jeu::message] #[derive(Copy, Eq, Variantes)]
pub enum Etape { Salon, Essais, Revelation }
impl Phase for Etape { fn terminale(self) -> Oui { Oui::Non } }

#[jeu::message] pub enum GesteTel { Rejoindre(Texte<Prenom>), Proposer }
#[jeu::message] pub enum ActionEcran { Lancer, Rejouer }
#[jeu::message] pub enum Avis { PasMaintenant, DejaPropose, TropTard }
#[jeu::message] pub enum VueTel { Prenom, Attends, Choisis { curseur: Chiffre }, Resultat { nombre: Chiffre, gagne: Oui } }

/// Le chiffre que le joueur fait défiler : un tenu, le dernier gagne.
#[jeu::message] pub struct Curseur(pub Chiffre);
impl Tenu for Curseur {
    type Cle = ();
    fn cle(&self) {}
    fn relache(&self) -> Option<Self> { None }   // un curseur ne se relâche jamais seul
}

impl Jeu for Devine {
    type ActionEcran = ActionEcran;
    type GesteTelephone = GesteTel;
    type TenuTelephone = Curseur;
    type VueTelephone = VueTel;
    type Avis = Avis;
    type Phase = Etape;
    type Garde = Appareils<Joueur>;
    type Precedent = Aucun;

    const DECLARATION: Declaration = Declaration {
        nom: "devine",
        places: Places::joueurs(1, 8).appareils(16),
        rattrapage: 1,
        fenetre: Applicable::SansObjet(raison!("tour par tour, le pas vu suffit")),
        antidatage_max: Duree::ms(30),
        pause: Pause::Aucune(raison!("chaque phase a une échéance")),
        ecran: FormeEcran::Scene { largeur: 16, hauteur: 9, zone_sure_pour_cent: 90, echelle_entiere: Oui::Non },
        telephones: Applicable::Oui(Telephones {
            repartition: Repartition::UnParJoueur,
            tenu: ReglesTenu {
                fraicheur: Applicable::SansObjet(raison!("le curseur reste où le joueur l'a laissé")),
                age_max: Applicable::SansObjet(raison!("seul le dernier curseur compte, même tardif")),
                debit_max: 10,
            },
        }),
        manettes: Applicable::SansObjet(raison!("aucune manette n'a de rôle ici")),
        prediction: Applicable::SansObjet(raison!("un échange toutes les quelques secondes suffit")),
        hasard: Applicable::Oui(()),
        secrets: Applicable::Oui(()),
        fin: Applicable::SansObjet(raison!("les manches s'enchaînent sans fin de partie")),
        capacites: Voulues { vibration: Oui::Non, mouvement: Oui::Non, son: Oui::Non, ecran_allume: Oui::Oui, micro: Oui::Non, pad_natif: Oui::Non },
        limites: Limites { geste: 256, file: 32, vue: 4096, effets: 8, memoire: 64, brouillon: 1 << 20 },
        diagnostic: CodeTouche::new("KeyD"),
        assets: &[],
        mecaniques: &[Mecanique::Hasard, Mecanique::Texte, Mecanique::TourParTour],
    };

    fn creer(mut c: Creation<'_, Self>) -> Self {
        let nombre = match c.hasard() { Ok(h) => h.dans::<0, 9>(), Err(Absent) => Chiffre::PLUS_PETIT };
        let pas = c.maintenant().pas();
        Devine {
            phase: Etape::Salon, phase_depuis: pas, fin: None,
            nombre: Secret::cacher(Qui::Personne, nombre),
            joueurs: c.garde().unwrap_or_else(Appareils::new),
            manche: 0, lignes: Vec::new(),
        }
    }

    fn appliquer(avant: Avant<'_, Self>, e: Entree<'_, Self>) -> Issue<Self> {
        match e {
            Entree::Geste(Geste { de, geste: GesteTel::Proposer, quand }) => {
                let j = avant.jeu();
                if j.phase != Etape::Essais || quand.vu < j.phase_depuis { return avant.refuser(Avis::TropTard); }
                let Some(joueur) = j.joueurs.get(de) else { return avant.refuser(Avis::PasMaintenant) };
                match joueur.essai.lire(avant.autorite()) {
                    Ok(Some(_)) => return avant.refuser(Avis::DejaPropose),
                    Ok(None) => {}
                    Err(a) => return avant.attendre(a),
                }
                let mut e = avant.accepter();
                let p = e.parts();
                if let Some(joueur) = p.jeu.joueurs.get_mut(de) {
                    let curseur = joueur.curseur;
                    if let Ok(essai) = joueur.essai.lire_mut(p.autorite) { *essai = Some(curseur); }
                }
                e.fin()
            }
            Entree::Geste(Geste { de, geste: GesteTel::Rejoindre(prenom), .. }) => {
                if prenom.vide() == Oui::Oui { return avant.refuser(Avis::PasMaintenant); }
                let mut e = avant.accepter();
                let joueur = Joueur { prenom, essai: Secret::cacher(Qui::Appareil(de), None), curseur: Chiffre::PLUS_PETIT, points: 0, present: Oui::Oui };
                let _: Option<Joueur> = e.jeu().joueurs.inserer(de, joueur);
                e.fin()
            }
            Entree::Tenu(Changement { de, valeur: Curseur(c), .. }) => {
                if avant.jeu().joueurs.get(de).is_none() { return avant.ignorer(); }
                let mut e = avant.accepter();
                if let Some(j) = e.jeu().joueurs.get_mut(de) { j.curseur = c; }
                e.fin()
            }
            Entree::Ecran(DeEcran::Action { action: ActionEcran::Lancer, quand }) => {
                let j = avant.jeu();
                if j.phase == Etape::Essais || quand.vu < j.phase_depuis { return avant.ignorer(); }
                let mut e = avant.accepter();
                let p = e.parts();
                let nouveau = match p.sorties.hasard() { Ok(h) => h.dans::<0, 9>(), Err(a) => return e.attendre(a) };
                p.jeu.ouvrir_manche(&p.maintenant, nouveau, p.autorite);
                e.fin()
            }
            Entree::Temps(_) => {
                let j = avant.jeu();
                let Some(fin) = j.fin else { return avant.ignorer() };
                if fin.atteinte(&avant.maintenant()) == Oui::Non { return avant.ignorer(); }
                let mut e = avant.accepter();
                let p = e.parts();
                p.jeu.reveler(&p.maintenant, p.autorite);
                let id = p.sorties.id();
                p.jeu.lignes.push((id, p.jeu.ligne_de_resultats()));
                e.fin()
            }
            Entree::Reponse(id, Reponse::Fait(_)) => {
                let mut e = avant.accepter();
                e.jeu().lignes.retain(|(i, _)| *i != id);   // un échec ne bloque rien (SQ-78)
                e.fin()
            }
            Entree::Presence(Presence::Depart { appareil, .. }) => marquer(avant, appareil, Oui::Non),
            Entree::Presence(Presence::Retour { appareil, .. }) => marquer(avant, appareil, Oui::Oui),
            Entree::Presence(Presence::Arrivee { .. } | Presence::Capacites { .. })
            | Entree::Ecran(DeEcran::Action { action: ActionEcran::Rejouer, .. })
            | Entree::Ecran(DeEcran::Touche { .. } | DeEcran::Pointeur { .. } | DeEcran::Composant { .. })
            | Entree::Reponse(_, Reponse::Image(_) | Reponse::Rendu(_) | Reponse::Lu(_))
            | Entree::Manette(_) => avant.ignorer(),
        }
    }

    fn vue_ecran(&self, r: &Regard, s: &mut Scene<ActionEcran>) {
        if let Some(n) = self.nombre.voir(r) { s.texte(Cle::de(&("nombre", self.manche)), chiffre(n), GRAND, STYLE); }
        if let Some(fin) = self.fin { s.compte_a_rebours(Cle::de(&"temps"), CompteARebours::Court { echeance: fin }, HAUT, STYLE); }
        s.bouton(Cle::de(&"lancer"), BAS, "Lancer (Espace)", ActionEcran::Lancer);
        s.raccourci(CodeTouche::ESPACE, Mods::AUCUN, Oui::Non, ActionEcran::Lancer);
        s.effet(Effet { id: IdEffet::de(&("phase", self.manche, self.phase)), quand: Instant::de(self.phase_depuis),
                        quoi: Sensation::Son(Son::Signal(Signal::Avance)), tenue: Tenue::Definitive, peremption: Duree::ms(250) });
    }
    fn vue_telephone(&self, r: &Regard) -> Montre<VueTel> { /* … depuis `r.appareil()` et `voir(r)` */ }
    fn ressource(&self, _: Cle) -> Option<&[u8]> { None }
    fn phase(&self) -> Etape { self.phase }
    fn reveil(&self) -> Reveil { self.fin.map_or(Reveil::Jamais, Reveil::A) }
    fn attente(&self, a: &mut Attente<'_, Self>) {
        match self.fin { Some(fin) => a.echeance(fin), None => a.ecran(&[ActionEcran::Lancer]) }
    }
    fn accueil(&self) -> Accueil { if self.joueurs.len() >= 8 { Accueil::Pleine } else { Accueil::Ouverte } }
    fn demandes<'s>(&'s self, d: &mut Demandes<'s>) {
        for (id, ligne) in &self.lignes { d.ajouter(*id, || Demande::Ligne { flux: RESULTATS, octets: ligne }); }
    }
    fn previsible(_: &GesteTel) -> Prevision { Prevision::Non }
    fn garder(&self) -> Appareils<Joueur> { self.joueurs.clone() }
    fn convertir(a: Aucun) -> Result<Self, Raison> { match a {} }
}

impl Epreuve for Devine {
    type Resume = Vec<u32>;                     // les points, dans l'ordre d'arrivée
    type Cas = Etape;
    fn resume(&self, _: &Autorite) -> Vec<u32> { self.joueurs.iter().map(|(_, j)| j.points).collect() }
    fn invariants(&self, _: &Autorite, v: &mut Verif) {
        jeu::exiger!(v, self.joueurs.iter().all(|(_, j)| j.points <= self.manche), "au plus un point par manche");
        jeu::exiger!(v, self.lignes.len() <= 2, "les lignes écrites se retirent");
    }
    fn transition(avant: &Vec<u32>, apres: &Vec<u32>, v: &mut Verif) {
        jeu::exiger!(v, avant.iter().zip(apres).all(|(a, b)| b >= a), "un point ne se perd pas");
    }
    fn bot_telephone(_: &Self, m: &Montre<VueTel>, b: &mut Bot<'_, Self>) {
        match &m.vue {
            VueTel::Prenom => { let t = b.texte_hostile(); b.geste(GesteTel::Rejoindre(Texte::nettoyer(t))) }
            VueTel::Choisis { .. } if b.depuis_vue() > Duree::ms(800) => {
                let c = b.hasard().dans::<0, 9>();
                b.tenir(Curseur(c));
                b.geste(GesteTel::Proposer);
            }
            VueTel::Choisis { .. } | VueTel::Attends | VueTel::Resultat { .. } => b.rien(),
        }
    }
    fn bot_ecran(j: &Self, b: &mut Bot<'_, Self>) {
        if j.fin.is_none() && b.depuis_vue() > Duree::s(2) { b.action(ActionEcran::Lancer) } else { b.rien() }
    }
    fn cas(&self) -> Etape { self.phase }
    fn pire_cas(c: Etape) -> Self { /* 8 prénoms de 12 capitales larges, points à 9 999, pour chaque étape */ }
    fn proprietes() -> Proprietes<Self> {
        Proprietes {
            coupure_transparente: Applicable::Oui(Coupure { moins_de: Duree::s(10), hypothese: |j, a, qui| j.a_deja_propose(a, qui) }),
            permutation: Applicable::Oui(()),
        }
    }
}

/// Un départ ou un retour : le jeu note la présence, et ne libère rien (le commun non plus).
fn marquer(avant: Avant<'_, Devine>, a: Appareil, present: Oui) -> Issue<Devine> {
    if avant.jeu().joueurs.get(a).is_none() { return avant.ignorer(); }
    let mut e = avant.accepter();
    if let Some(j) = e.jeu().joueurs.get_mut(a) { j.present = present; }
    e.fin()
}
```

Les corps marqués `/* … */`, les fonctions d'aide (`ouvrir_manche`, `reveler`, `ligne_de_resultats`, `a_deja_propose`, `chiffre`) et les constantes de mise en page (`GRAND`, `HAUT`, `BAS`, `STYLE`) sont omis.

Ce que l'exemple montre de la forme : « déjà proposé » est refusé avant `accepter`, donc sans toucher au générateur ; un chiffre tiré n'existe qu'après `accepter` ; la télé ne voit le nombre qu'une fois `montrer(Qui::Tous)` fait par `reveler` ; l'échéance de la manche naît de `Maintenant` dans `ouvrir_manche` ; la ligne de résultats est une demande, écrite une fois, et retirée à sa réponse ; un geste d'une manche passée tombe sur `quand.vu`.

---

## 10. Les trois jeux et un FPS

| | Punchline | Sidequest | Tier list | FPS sur téléphones |
|---|---|---|---|---|
| Temps | `Reveil::A` : la plus proche des échéances, robots et grâces | `ChaquePas`, `rattrapage: 3` | `Jamais`, `ChaquePas` pendant un glisser | `ChaquePas` |
| Fenêtre | sans objet : `quand.vu` suffit | sans objet d'abord ; 6 pas à essayer (question 6) | sans objet | 12 pas, à mesurer |
| Tenu | `Brouillon`, clé `(consigne, case)`, jamais relâché, débit 4 | `Direction(Axe2)`, fraîcheur 450 ms, âge 1 s | aucun | `Commandes { deplacement: Axe2, visee: Cumul2, tir: Bouton, saut: Bouton }` |
| Secrets | réponses (`Qui::Appareil` de l'auteur), auteurs et bulletins (`Personne`), brouillons ; `montrer` au vote et à la révélation | aucun | aucun | positions hors de vue : `Qui::Appareils` de ceux qui ont une ligne de vue |
| Hasard | paquets, ordre des duels, `{joueur}`, réponses de secours ; refus d'abord | aucun ; le code de salle vient du commun | aucun | apparitions |
| Demandes | aucune | une `Ligne` par manche, jusqu'à sa réponse | `Image` par fichier, `Rendu` de l'export, `Lire` d'une sauvegarde, `Telecharger` ; Reset vide la liste, donc annule tout | aucune |
| Écran | `Scene` 16 × 9, zone sûre 90 % | `Scene` 1280 × 760, `pixel_entier`, `glisse` par la clé du rang | `Page`, `Deplacables`, `Champ`, `EnPlace`, `Selecteur`, `Modale`, `Lien` | `Scene`, `glisse`, `lisse`, `definitif` pour les éliminations |
| Effets | un signal par transition, identité `(n, signal)`, `Definitive` | vibration de point gagné, identité `(manche, appareil)`, `Definitive` | aucun | son de tir `Provisoire`, identité `quand.id` ; marqueur de touche `Definitive` |
| Nouvelle partie | « On rejoue ! » : `garde` = joueurs et paquets | `Rejouer` : `garde` = personnages, rangs, numéro de manche | sans objet | revanche |
| Prédiction | non | non : la télé est sur l'autorité | non | oui : déplacement et tir de l'appareil |

**Ce qu'un FPS exige, et que l'interface porte.** La visée en `Cumul` : un paquet perdu ne la décale pas. Le tir en `Bouton.appuis` : deux tirs dans un envoi comptent deux. Un tir jugé sur ce que voyait le tireur : `quand.vu` et `quand.fait.fraction` désignent son image, le jeu garde un anneau de positions passées. La balle prédite et la confirmée nommées depuis `quand.id`. Une élimination dans `definitif`. Un ennemi caché dans un `Secret` : sur le téléphone, une prédiction qui le touche rend `Absent`, et la vision à travers les murs est impossible par la forme. Le coût : `fenêtre` pas rejoués par image où un geste arrive en retard, mesuré par E19.

**Ce que les cahiers changent.** Punchline : rien, la politique de gel est celle du cahier. Sidequest : la croix (1, 1) est ramenée à une norme 1 par `Axe2`, et les trajectoires de référence restent à vérifier à 1e-6 près ; les relâchements forcés (pause, fin de manche, Fin, arrivée, retour) sont des règles de l'étape (`dir = Axe2::ZERO`), ceux du monde (silence, départ, arrière-plan, âge) sont du commun ; le saut aux Résultats se tranche dans l'étape, une seule fois pour la page et le pad (SQ-26). Tier list : l'historique est dans l'état, donc il survit au rechargement ; les images vivent dans la mémoire, partagées par `Partage`.

---

## 11. Ce que la forme rend impossible

« Impossible » veut dire : le programme fautif ne compile pas, la valeur fautive ne se construit pas, ou le moteur l'empêche quoi que fasse le module.

| Famille de bugs | Par quelle forme | Leçons |
|---|---|---|
| Un refus qui change l'état ou consomme du hasard | `Avant` ne donne que `&J` ; hasard et sorties n'existent qu'après `accepter` | PUN-27, OUT-171, HIS-44, CAT-94, SQ-25, TL-02 |
| Une vue, un bot ou un invariant qui modifie l'état | `&self` ; allocations au brouillon ; empreinte constatée | TL-01 |
| Une entrée fautive rejouée en boucle par la reprise | l'entrée est écrite `Ecartee` et sautée | CONTRAT, « La forme d'un jeu » |
| Un état à moitié modifié après une panique | restauration du point et rejeu du journal | SAL-140, HIS-49 |
| Un pas qui ne rend pas la main | carburant compté dans le module | CONTRAT, « Chaque pas a un budget » |
| Une faute de budget ou de pile qui ne se rejoue pas | carburant et profondeur en instructions | SQ-59, SQ-60 |
| NaN, infini, valeur hors bornes venus d'un appareil | `Fini`, `Axe`, `Axe2`, `Borne`, `Texte` se valident au décodage | HIS-45, OUT-175, CAT-04, CAT-22, SQ-67 |
| NaN dans une vue | longueurs en `Fixe` | REM-57 |
| NaN qui ne se rejoue pas au bit près | NaN canoniques à la réécriture | SQ-68 |
| Octets restants, longueur absurde, désérialiseur tolérant | décodage strict ; attributs `serde` refusés | OUT-146, REM-58 à REM-60, SAL-118, SAL-119, PUN-91 |
| Le jeu lit l'heure, le hasard du système, un fichier | aucun import, `no_std` | OUT-145, PUN-89, SAL-93 |
| `HashMap`, `sin`, `powf` qui diffèrent entre natif et module | absents de `no_std` | HIS-50, OUT-144 |
| Un secret dans la vue d'un téléphone ou de la télé | `Secret` n'est pas `Message` ; une vue ne voit qu'à travers son `Regard` | CAT-46, SQ-64 |
| Une prédiction qui devine un tirage | le hasard est un secret de l'enveloppe | CONTRAT, « Réagir sans attendre » |
| Une prédiction qui décide | la définitive ne reçoit que des gestes | SAL-194 |
| Une échéance chaînée sur l'échéance d'avant un gel | `Echeance` ne naît que de `Maintenant` | PUN-34 |
| Une échéance ou un minuteur qui survit à sa partie | pas de minuteur ; nouvelle partie = instance neuve | CAT-71, CAT-73, CAT-75 |
| Une spirale après un gel | rattrapage plafonné, temps jeté et dit | CAT-15, SQ-62 |
| Une règle qui dépend de l'identité d'un appareil | `Appareil` sans ordre ni hachage, tiré au hasard ; égalités par la graine | CAT-27 |
| Une réponse à une demande annulée ou remplacée | demandes lues dans l'état, réponse filtrée | TL-44 à TL-46, SAL-164 |
| Une ligne de résultats écrite deux fois | demandes de la seule définitive, au plus une fois par identité | CAT-76 |
| Un effet rejoué par un réaffichage, une reconnexion, un rejeu | effet dans la vue, avec identité, instant et péremption | PUN-66, SQ-79, SAL-167, SAL-155 |
| Un résultat montré puis retiré | `definitif` et `Tenue::Definitive` lus sur la définitive, par clé | CAT-41 |
| Un geste d'une phase passée appliqué à la suivante | `quand.vu`, rempli et borné par l'autorité | PUN-38, SQ-23, SAL-115, CAT-44 |
| Un appareil qui antidate | `fait` borné par la latence mesurée | SAL-129 |
| Une direction vieille qui fait glisser un personnage | âge et fraîcheur tenus par le commun ; un relâchement passe toujours | SQ-01, SQ-02, CAT-25 |
| Des états périmés livrés d'un bloc | numéro croissant, dernier gagnant | REM-13, REM-14 |
| Un appui perdu ou compté deux fois | événements une fois ; `Bouton.appuis` en `Cumul` | REM-16, REM-17, CAT-16, CAT-17, SQ-99 |
| Un téléphone qui envoie une commande d'organisateur | `ActionEcran` et `GesteTelephone` sont deux types ; l'écran n'est pas un `Appareil` | SQ-21, SQ-22, PUN-48, SAL-33 |
| Une action que l'écran n'affichait pas | l'action doit figurer dans la dernière scène | SAL-152 |
| Un lien, une session, un onglet dans le jeu | le jeu ne voit que des `Appareil` | SAL-14, SAL-15, SAL-18 |
| Un texte trop long, coupé dans un graphème, invisible | `Texte<L>` nettoyé et coupé, validé au décodage | TL-26, TL-28, PUN-23, PUN-24, CAT-83, CAT-85 |
| Une limite différente dans la page et dans les règles | `Limite` : une seule définition | TL-27, SQ-40 |
| Une exemption silencieuse | `Applicable` sans `Default`, champs obligatoires, `Raison` vérifiée | HIS-04, HIS-43, OUT-03 à OUT-05 |
| Une déclaration incohérente | `Places` en `const fn` ; accord déclaration et types en `const` | OUT-200, HIS-13 |
| Une capacité dont le jeu dépend | il n'existe pas de champ pour en exiger une | REM-146, OUT-211 |
| Une page ou une sauvegarde d'une autre version lue de travers | `formes`, empreinte du module, conversion par `Precedent` | SAL-41, PUN-44, REM-77, HIS-51, TL-13 |
| Une sérialisation de l'état écrite à la main | la mémoire est l'état ; la forme canonique est dérivée | CONTRAT, « Sauvegardes » |
| Un bot qui joue ce que l'écran n'offre pas, ou triche | il ne voit que son regard et sa vue ; ses gestes passent par le même chemin | PUN-76, SAL-10 |

**Ce qui reste aux pressions.** Copier un secret dans un champ public trop tôt (E8). Une règle fausse, un score faux avec des types justes (invariants, oracles indépendants, mutations). `Echeance::repoussee` détournée pour chaîner (E18). Une mise en page qui déborde (galerie, avec le rendu). Une fuite de mémoire dans l'état (E15). Un jeu qui n'oublie personne (E15). Une résolution simultanée inéquitable (CAT-28, au jeu).

---

## 12. Les décisions

| # | Décision | Raison | Origine |
|---|---|---|---|
| D1 | Une seule fonction d'étape, en deux temps : `Avant` lit, `Apres` écrit | le contrat dit « le jeu lit l'ancien état, et ne produit un nouvel état que s'il accepte » ; le compilateur le fait respecter sans copie de mémoire avant chaque entrée | temps réel |
| D2 | Le moteur ne restaure la mémoire qu'après une faute, depuis un point et le journal | une copie par entrée coûterait des dizaines de Mo à Tier list ; le suivi des pages écrites demande une seconde mémoire dont le support dans Safari n'est pas acquis | temps réel |
| D3 | Le journal garde les refus | leur rejeu refait les mêmes allocations : la mémoire reste identique entre deux exécutions | nouveau |
| D4 | Les lectures allouent au brouillon, remis à zéro | une vue appelée un nombre de fois différent ne change pas la mémoire ; l'empreinte compare deux exécutions | temps réel |
| D5 | Aucun import ; deux exports ; une enveloppe `Appel`/`Retour` | le constat est une section vide ; une seule porte à fuzzer et à prouver ; la richesse se versionne dans les types | plus petit noyau, types |
| D6 | `jeu-abi` sans type de jeu, dont dépendent moteur et cœur | le moteur ne connaît aucun jeu ; une seule définition ; une seconde lecture indépendante dans les tests | types |
| D7 | `no_std` pour le jeu et `jeu` | quatre règles d'écriture deviennent des formes | plus petit noyau, types |
| D8 | La crate du module, de deux lignes, hors du jeu | la crate du jeu reste `forbid(unsafe_code)` | types |
| D9 | Vues, effets, demandes, réveil, attente lus dans l'état | une image recalculée depuis l'état ne peut pas s'en désynchroniser ; une demande retirée est annulée par la forme | plus petit noyau, types |
| D10 | Sorties d'une étape réduites à `oublier`, `retirer`, `nouvelle_partie` | ce sont des décisions ponctuelles, pas des états ; une `Salle` déclarative retirait un appareil oublié par erreur | types |
| D11 | Effet avec identité, instant, tenue et péremption | sans tenue, un son de tir rejoue à chaque rejeu de la tête ; sans péremption, un absent rejoue en retard | temps réel |
| D12 | Deux instances, définitive et tête, quand une fenêtre existe | un résultat ne s'annonce qu'une fois définitif ; demandes et sorties ne partent que du définitif ; sans fenêtre, une seule instance | temps réel |
| D13 | 60 pas par seconde pour tous | `Duree::ms` reste une constante ; aucun des trois jeux ni le FPS esquissé n'en demande plus | types |
| D14 | Après un gel, le numéro de pas saute au-delà du rattrapage | les appareils partagent des numéros de pas qui suivent l'heure ; avec `Echeance` née de `Maintenant`, une seule phase se termine, comme le cahier de Punchline le veut | plus petit noyau |
| D15 | `Maintenant` qu'on ne garde pas, `Echeance` qui n'en naît que | la phase suivante part du réveil, par la forme | types |
| D16 | `Quand { place, fait, recu, vu, id }` sur chaque geste | refuser le périmé sans numéro de phase écrit à la main ; départager une course ; nommer ce qu'un geste crée | temps réel |
| D17 | `Tenu` avec clé et relâchement, règles déclarées, tenues par le cœur | fraîcheur, âge et débit sont du monde, pas du jeu ; un brouillon et un stick ont la même forme | temps réel, plus petit noyau |
| D18 | `Bouton` et `Cumul` dans le vocabulaire | un état ne compte pas les appuis ; une visée en écarts dérive | temps réel |
| D19 | `Appareil` sans ordre ni hachage, `Appareils<V>` dans l'ordre d'insertion | un jeu ne peut pas départager par l'identité ; reste la pression de permutation | types |
| D20 | `Secret<T>` avec `Qui` ; `lire` par `Autorite`, `voir` par `Regard` | la télé passe par son regard sans copie ; pas de cellule globale cachée dans l'enveloppe | temps réel |
| D21 | Le hasard dans l'enveloppe, en secret, atteint par `Sorties` | jamais avant d'accepter ; jamais projeté ; jamais prédit | types, temps réel |
| D22 | `Texte<L: Limite>` nettoyé dans le module, validé au décodage | une limite, une unité, un nettoyage pour la page et les règles ; une version d'Unicode par module | types, temps réel |
| D23 | Images et fichiers dans la mémoire, partagés par `Partage` | le contrat dit que la mémoire entière est l'état ; des blocs hors mémoire l'étendraient (question 1) | plus petit noyau |
| D24 | Forme canonique dérivée et scellée | sauvegarde d'une version à l'autre, comparaison avec la copie native, perturbations de l'épreuve ; le jeu ne sérialise jamais un secret | temps réel, types |
| D25 | Page téléphone écrite par le jeu, dans sa crate | le contrat dit « chaque jeu code la page de téléphone qu'il veut » ; une page déclarative le contredirait | plus petit noyau, temps réel |
| D26 | La scène de l'écran est une donnée du commun, actions comprises | une action de l'écran ne peut être que celle que la scène portait | types |
| D27 | `Applicable<T>` avec `Raison` vérifiée à la compilation ; aucun `Default` | rien ne devient sans objet en silence | les trois |
| D28 | Accord déclaration et types vérifié en `const` | une incohérence casse la compilation, pas la soirée | types |
| D29 | Bots dans le module, qui voient leur regard et leur vue | même binaire que la partie ; un bot ne triche pas et ne joue que ce qu'un humain verrait | temps réel, plus petit noyau |
| D30 | `Resume` plutôt que deux états pour `transition` | le module n'a pas à garder une copie de l'état | plus petit noyau |
| D31 | Attente avec des exemples de gestes | « jamais figée » se vérifie par un essai sur une copie, pas par une supposition | temps réel |
| D32 | Carburant, profondeur, NaN et croissance réécrits dans le module | le même module réécrit se comporte pareil dans wasmtime, dans un navigateur et dans un interpréteur | les trois |
| D33 | Le cœur générique sur `Instance`, ses règles en fonctions pures | testé sur un modèle sans WebAssembly ; la preuve vise `regles` d'abord | nouveau |
| D34 | `dlmalloc` pour le tas, une arène pour le brouillon | un allocateur mûr plutôt qu'un `unsafe` neuf ; l'arène est assez petite pour être prouvée | nouveau |
| D35 | SIMD refusé dans cette version | sa canonisation des NaN n'est pas écrite, et aucun jeu ne la demande | temps réel |
| D36 | La déclaration lue par `Decrire`, pas dans une section | rien à écrire après la compilation pour la phase 1 | plus petit noyau |
| D37 | Gabarits de gestes écartés | ils supposent une page décrite par le commun, que le contrat ne veut pas, et l'ordre des octets de postcard | — |
| D38 | Suivi des pages écrites écarté pour la phase 1 | coût non mesuré, seconde mémoire incertaine dans Safari ; il pourra accélérer points et restaurations sans changer l'interface | — |

---

## 13. Questions pour le mainteneur

1. **Les gros états.** Les images de Tier list (des dizaines de Mo) restent dans la mémoire (D23). Un point de reprise copie alors toute la mémoire, et la garder hors de l'onglet coûte cher. Faut-il accepter des blocs immuables hors de la mémoire, nommés par leur empreinte, ce qui étend « la mémoire entière est son état » ?
2. **Le suivi des pages écrites.** Faut-il le prévoir pour la phase 2, comme accélération des points et des restaurations, en réservant une zone dans la mémoire du jeu plutôt qu'une seconde mémoire ?
3. **La fréquence.** 60 pas par seconde pour tous (D13) : faut-il une fréquence déclarée par jeu, pour un jeu de rythme ?
4. **Le gel.** Le numéro de pas saute au-delà du rattrapage (D14). L'autre voie fait glisser le temps de partie, et la phase en cours continue au lieu de se terminer. Laquelle veut-on ?
5. **La prédiction des autres.** Sur un appareil qui prédit, `Temps` fait avancer tout le monde, avec les tenus visibles de la projection. Le contrat dit que seules les conséquences du geste de l'appareil s'affichent avant confirmation. Accepte-t-on cette extrapolation, ou faut-il que la vue ne montre d'avance que ce qui vient de l'appareil ?
6. **La fenêtre de Sidequest.** Zéro d'abord, ou 6 pas pour placer un relâchement à son vrai moment ? À juger sur la vraie télé.
7. **Les relâchements forcés de Sidequest.** À la reprise d'une pause, la direction tenue repart-elle ou est-elle relâchée ? Le cahier et la conception divergent ; c'est une règle de l'étape.
8. **La configuration au lancement.** Aucun `Config` n'est passé à `creer` : le contenu est compilé dans le jeu, et un fichier passe par une demande `Lire`. Un paquet de questions choisi au lancement en a-t-il besoin ?
9. **Le second onglet de Tier list.** Un second regard sur l'écran, en lecture seule, qui reprend la main d'un geste, ou un refus ?
10. **Les gestes qu'exige le navigateur.** Ouvrir un sélecteur ou lancer un téléchargement demande un geste du joueur, que l'aller-retour vers le Worker perd. Le composant les fait-il lui-même, sans passer par l'étape, comme `Lien` le fait ?
11. **Les rôles.** Trois rôles (écran, téléphone, manette) suffisent-ils, ou un jeu doit-il pouvoir déclarer « équipe » ou « arbitre » ?
12. **Les budgets.** Carburant par sorte d'appel, profondeur maximale, intervalle des points : à mesurer sur les trois jeux reconstruits, puis à écrire dans `CONTRAT.md`. Qui décide des valeurs ?
13. **Un second moteur dans l'épreuve.** Faut-il, dès la phase 1, un interpréteur (`wasmi`) à côté de wasmtime ? Il ferait de « deux moteurs » une comparaison réelle, et ressemble au cas de l'app iPhone.
14. **Les touches du commun.** `F8` marque ; la touche de diagnostic est déclarée par le jeu (Sidequest garde `D`). Est-ce assez ?
15. **Le code de salle.** Sidequest nomme ses résultats par un code de salle stable. Le commun le donne-t-il au jeu dans `Creation`, ou l'ajoute-t-il seul aux lignes du flux ?
16. **Les sauvegardes de l'ancien Tier list.** Lire les fichiers au format 1 déjà téléchargés demande une conversion écrite par le jeu, hors de la forme canonique. La veut-on ?
17. **Les effets provisoires.** Leur annulation coupe-t-elle un son déjà parti, ou empêche-t-elle seulement de le jouer ?
18. **La première preuve.** « Ne jamais planter la partie » : l'énoncé porte-t-il sur les `regles` du cœur, sur la colle de `jeu`, ou sur l'étape d'un jeu ?
19. **La déclaration dans le module.** Faut-il, plus tard, l'écrire dans une section personnalisée, lisible sans rien exécuter ?
20. **L'emplacement.** Les crates vivent-elles dans `soiree/crates/`, en un seul workspace, et le nom `jeu` reste-t-il, sachant qu'il serait pris sur crates.io ?
