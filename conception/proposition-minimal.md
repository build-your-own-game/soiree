# Proposition : le plus petit noyau

> Une proposition pour l'interface entre le commun et un jeu : la crate `jeu`, et le contrat binaire entre le module WebAssembly d'un jeu et le moteur du commun. Elle suit `MANIFESTE.md` et `CONTRAT.md`, qui font foi. Son angle : garder le noyau le plus petit possible, et construire tout le reste par-dessus, dans le commun. À chaque besoin, la question posée a été : peut-on le retirer du noyau ?

## L'idée en cinq phrases

1. Le module exporte sa mémoire et **une seule fonction**, `etape`. Il n'importe rien.
2. Un appel, c'est des octets écrits dans la mémoire, une exécution, des octets lus. Le moteur décide ensuite s'il **garde** la mémoire ou s'il la **remet** telle qu'avant l'appel.
3. Le moteur ne garde la mémoire que pour une entrée acceptée. Un refus, une lecture, une faute, un pas trop long : tout le reste est remis. Un refus ne change rien, une lecture ne change rien, une faute ne laisse rien : c'est un seul mécanisme.
4. L'étape ne rend rien d'autre que « accepté » ou « refusé ». Tout ce que le commun doit savoir (vues, effets, demandes, qui on attend, quand se réveiller, qui garder dans la salle) se **lit dans l'état**, par une lecture qui est remise ensuite.
5. Le reste (prédiction, rejeu, reprise, épreuve, dessin, composants) est construit par le commun au-dessus de ces deux gestes : appeler, garder ou remettre.

Ce qui a été retiré, et pourquoi, est résumé à la fin de la section 2.

---

## 1. L'API Rust de la crate `jeu`

### 1.1 La forme de la crate

- `jeu` est `#![no_std]`, avec `alloc`. Un jeu l'est aussi, hors de ses tests.
  - Raison : sans `std`, `HashMap`, `Instant::now`, `SystemTime`, les fichiers, les fils et les fonctions flottantes de la plateforme (`sin`, `powf`…) n'existent pas. Quatre règles d'écriture deviennent des formes : le compilateur les fait respecter (OUT-145, HIS-50).
  - Les tests et la copie native ont `std` (`#[cfg(test)] extern crate std;`).
- `jeu` fournit, pour le module seulement : l'allocateur, le gestionnaire de panique (`panic` devient le piège `unreachable`), et la colle d'export.
- Les dépendances de `jeu` : `serde` et `postcard` (sans `std`), `libm`. La macro dérivée vit dans `jeu-derive`. Le langage de dessin vit dans la crate `dessin`, dont `jeu` réexporte un seul type, `Scene`.
- Tout ce qui suit est public, sauf ce qui est marqué « caché ».

### 1.2 Les valeurs venues de l'extérieur

Chaque type qui franchit la frontière se valide à son décodage. Une valeur invalide ne se décode pas : elle n'atteint jamais l'étape (HIS-45, OUT-175).

```rust
/// Un f32 fini et non sous-normal. Ni NaN, ni infini.
#[derive(Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize /* valide */)]
pub struct Fini(f32);
impl Fini {
    pub const fn zero() -> Self;
    pub fn new(x: f32) -> Option<Self>;   // None pour NaN, ±inf, sous-normal
    pub fn get(self) -> f32;
}

/// Un axe dans [-1, 1], zone morte déjà appliquée par le commun sur l'appareil.
pub struct Axe(Fini);
/// Deux axes de norme au plus 1.
pub struct Stick { pub x: Axe, pub y: Axe }

/// Un texte nettoyé par le commun : NFC, contrôles et Cf retirés, espaces réduites,
/// coupé entre graphèmes à la limite N, comptée dans l'unité M.
/// Deux textes égaux au sens du commun ont la même `Cle`.
pub struct Texte<const N: u32, M: Mesure>(/* caché */);
pub trait Mesure { /* caché */ }
pub enum PointsDeCode {}  pub enum Utf16 {}  pub enum Graphemes {}  pub enum Capitales {}
impl<const N: u32, M: Mesure> Texte<N, M> {
    pub fn as_str(&self) -> &str;
    pub fn est_vide(&self) -> Oui;           // jamais de bool en paramètre ; en retour, un enum
    pub fn cle(&self) -> Cle;                // casse, sélecteurs et liants ignorés
    pub fn nettoyer(brut: &str) -> Self;     // le même nettoyage, pour le contenu du jeu
}

/// Une raison écrite. Vide ou creuse (« n/a », « sans objet ») : refusée à la déclaration (HIS-43).
pub struct Raison(&'static str);
#[macro_export] macro_rules! raison { ($s:literal) => { /* vérifiée à la compilation : non vide */ } }

/// Le type sans valeur, pour ce qu'un jeu n'a pas. Un appareil ne peut pas l'envoyer.
pub enum Aucun {}

pub enum Oui { Oui, Non }
```

La limite d'un texte n'a qu'une définition : le jeu écrit `const PRENOM: u32 = 12;`, l'utilise dans `Texte<PRENOM, Capitales>` et dans le champ de sa page. Le commun mesure dans la même unité des deux côtés (TL-26 à TL-28, PUN-24).

### 1.3 Le temps, l'ordre et le hasard

```rust
/// Un numéro de pas sur l'horloge de la partie qui fait foi. Il ne recule jamais.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pas(u64);
impl Pas {
    pub fn plus(self, d: Duree, hz: Hz) -> Pas;     // saturant, arrondi au pas supérieur
    pub fn depuis(self, avant: Pas) -> u64;          // saturant
}
/// Une durée en millisecondes entières.
pub struct Duree(u32);
impl Duree { pub const fn ms(n: u32) -> Self; pub const fn s(n: u32) -> Self; }
pub struct Hz(u16);

/// La place d'une entrée acceptée dans l'histoire. Croît strictement.
/// Une vue porte l'ordre du dernier pas qu'elle montre ; un geste porte l'ordre de la vue
/// sur laquelle il a été fait.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Ordre { pas: Pas, rang: u32 }

/// Une heure UTC en millisecondes, donnée une fois, à l'ouverture.
pub struct Utc(u64);

/// 256 bits, neuve à chaque ouverture sauf rejeu voulu.
pub struct Graine([u8; 32]);
/// Le générateur du jeu. Il vit dans l'état : un refus le remet avec le reste.
#[derive(Clone)]
pub struct Hasard(/* ChaCha8, écrit dans le commun */);
impl Hasard {
    pub fn depuis(g: &Graine, flux: u64) -> Self;
    pub fn entier(&mut self, borne: u32) -> u32;     // sans biais
    pub fn melanger<T>(&mut self, t: &mut [T]);
}
```

### 1.4 Les appareils

```rust
/// Un appareil. Seul le commun le crée. Son ordre existe (pour les BTreeMap)
/// mais ne veut rien dire : les identifiants sont tirés au hasard (CAT-27).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Appareil(u64 /* constructeur caché */);

pub enum Role {
    /// L'écran de l'ordinateur de la soirée, son clavier et sa souris. Local, jamais réseau :
    /// un téléphone ne peut pas l'usurper.
    Ecran,
    Telephone,
    Manette,
}

/// Des capacités, jamais une technologie. Toutes optionnelles.
pub struct Capacites {
    pub vibration: Dispo,
    pub mouvement: Dispo,            // capteurs, dans les axes de l'interface
    pub son_sans_geste: Dispo,       // état audio réel
    pub ecran_allume: Dispo,
    pub micro: Dispo,                // jamais en http sur une adresse locale (OUT-211)
    pub orientation: Orientation,    // telle qu'elle est maintenant, demi-tour compris
    pub pad_natif: Dispo,
    pub prenom_suggere: Option<Texte<32, Graphemes>>,
}
pub enum Dispo { Oui, Non, ARefuser /* demandée, refusée par le joueur */ }
pub enum Orientation { Portrait, PortraitInverse, Paysage, PaysageInverse }

pub enum Presence {
    /// Un appareil jamais vu, ou oublié. Ses capacités présentes, pas seulement leurs changements.
    Arrive { appareil: Appareil, role: Role, capacites: Capacites },
    Part { appareil: Appareil, cause: Cause },
    Revient { appareil: Appareil, capacites: Capacites },
    Change { appareil: Appareil, capacites: Capacites },
}
pub enum Cause { Silence, Fermeture, Depart, ArrierePlan }
```

Le jeu ne voit ni lien, ni session, ni onglet, ni jeton. Un onglet jumeau, un rechargement, une app et sa page : un seul appareil (SAL-18 à SAL-21, SAL-36 à SAL-39).

### 1.5 Les entrées

```rust
pub enum Entree<'a, J: Jeu> {
    /// Le temps. Livré seulement quand le jeu l'a demandé (voir `Reveil`).
    Pas(Pas),
    Presence(Presence),
    /// Un événement : compté exactement une fois, dans l'ordre fixé par le commun.
    Geste(Geste<J::Geste>),
    /// Un état continu : le plus récent remplace les autres.
    Tenu(Tenu<J::Tenu>),
    /// La réponse à une demande encore listée par le jeu. Jamais pour une demande retirée.
    Reponse(DemandeId, Reponse<'a>),
}

pub struct Geste<G> {
    pub de: Appareil,
    pub role: Role,
    /// L'ordre de la vue que l'appareil montrait quand le geste a été fait.
    pub vu: Ordre,
    /// L'instant du geste, recalé sur l'horloge qui fait foi, borné : jamais avant
    /// `maintenant - age_max`, jamais après `maintenant`.
    pub fait: Pas,
    pub quoi: Action<G>,
}

/// Ce que le jeu définit (sa page, les boutons de sa scène), et ce que le commun capte.
pub enum Action<G> {
    Jeu(G),
    Touche(Touche),          // clavier de l'écran, hors des champs, sans répétition sauf demande
    Pointeur(Pointeur),      // souris, doigt et stylet par un même chemin
    Saisie(Saisie),          // un champ de texte de la scène
    Deplace(Deplace),        // le composant déplaçable de la scène
    Fichiers(Fichiers),      // choisis, déposés ou collés : nom, type, taille ; pas encore lus
}

pub struct Tenu<T> {
    pub de: Appareil,
    /// Croît à travers les reconnexions (REM-14). Le commun écarte les plus vieux.
    pub numero: u64,
    pub fait: Pas,
    pub recu: Pas,
    pub valeur: Lache<T>,
}
pub enum Lache<T> {
    Tient(Tenue<T>),
    /// Un relâchement passe toujours, même vieux.
    Lache(PourquoiLache),
}
pub enum Tenue<T> { Page(T), Stick(Stick) }
pub enum PourquoiLache { Geste, Silence /* bail dépassé */, ArrierePlan, Pause }
```

Les types `Touche`, `Pointeur`, `Saisie`, `Deplace` et `Fichiers` appartiennent au commun. Leur détail est en section 5. Ils sont validés comme le reste : position finie et dans la scène, champ qui existe dans la scène affichée, fichier dont la taille est connue avant toute lecture.

### 1.6 Les secrets et le lecteur

```rust
/// Qui lit. Le jeu ne peut pas en fabriquer : il n'en reçoit que des références,
/// pour la durée d'un appel. `Lecteur` n'est ni `Clone` ni `Copy`.
pub struct Lecteur { /* caché */ }
impl Lecteur {
    pub fn qui(&self) -> Qui;
    pub fn maintenant(&self) -> Pas;
    /// Le dernier pas définitif. Un résultat plus récent ne se montre pas encore.
    pub fn definitif(&self) -> Pas;
}
pub enum Qui { Autorite, Public, Appareil(Appareil) }

/// Une valeur que seuls l'autorité et `pour` peuvent lire.
#[derive(Clone)]
pub struct Secret<T> { pour: Pour, valeur: Option<T> }
pub enum Pour { Autorite, Appareil(Appareil) }
impl<T> Secret<T> {
    pub fn nouveau(pour: Pour, valeur: T) -> Self;
    pub fn pour(&self) -> Pour;                               // public : on sait qu'il existe
    pub fn lire(&self, l: &Lecteur) -> Result<&T, Absent>;
    pub fn ecrire(&mut self, ctx: &Contexte) -> Result<&mut T, Absent>;
    pub fn reveler(&self, ctx: &Contexte) -> Result<T, Absent> where T: Clone;
}
/// Le secret n'est pas lisible ici. Se convertit en `Refus::Absent` par `?`.
pub struct Absent;

/// La projection d'un état pour un lecteur : chaque secret illisible est vidé.
/// Dérivée, jamais écrite à la main.
pub trait Projeter { fn projeter(&self, l: &Lecteur) -> Self; }
pub use jeu_derive::Projeter;
```

`Secret<T>` n'implémente pas `Serialize`. Une vue est `Serialize`. Une vue ne peut donc pas contenir un secret : elle ne compile pas. Révéler, c'est copier la valeur dans un champ public, au moment voulu, dans l'étape.

### 1.7 Ce que le jeu montre et demande

```rust
/// Ce qu'un appareil reçoit : sa vue, ses effets, ses réglages.
#[derive(Serialize, Deserialize)]
pub struct Montre<V> {
    pub vue: V,
    /// Les effets ponctuels en cours. Le commun joue un effet la première fois qu'il voit
    /// son identité, à son instant, s'il n'est pas trop vieux. Un effet qui disparaît
    /// avant son instant est annulé.
    pub effets: Vec<Effet>,
    pub reglages: Reglages,
}
pub struct Effet { pub id: EffetId, pub a: Pas, pub quoi: QuoiEffet }
pub struct EffetId(pub u64);
pub enum QuoiEffet { Son(Son), Vibration(Vibration) }
pub enum Son { Signal(Signal) /* synthétisé par le commun */ }
pub enum Signal { Debut, Avance, Resultat, Fin }
pub enum Vibration { Courte, Succes }
pub struct Reglages {
    pub ecran_allume: Oui,
    pub fond: Option<Couleur>,
    pub orientation: Option<Orientation>,
    pub pad: Option<Pad>,             // le pad natif de l'app, s'il existe
}

/// Quand réveiller le jeu, qui il attend, où en est la partie.
pub struct Horloge { pub reveil: Reveil, pub attend: Attend, pub phase: Phase }
pub enum Reveil {
    /// Un pas à chaque pas (temps réel).
    ChaquePas,
    /// Un seul pas, à ce pas, ou au réveil s'il est passé. Une échéance est une donnée de l'état.
    A(Pas),
    /// Aucun pas : le jeu n'avance que sur des gestes.
    Jamais,
}
pub enum Attend {
    /// Ces appareils peuvent faire avancer la partie ; `echeance` la fait avancer sans eux.
    Appareils { qui: Vec<Appareil>, echeance: Option<Pas> },
    /// Rien n'avance seul, par nature, et le jeu dit pourquoi (un éditeur).
    Rien(Raison),
}
/// La progression, lue dans l'état (HIS-13, OUT-29).
pub struct Phase { pub nom: &'static str, pub numero: u64, pub fin: Oui }

/// Qui la salle garde. Le commun ne libère jamais une place seul.
pub struct Salle {
    pub accueil: Accueil,
    /// Les appareils que le jeu connaît. Un appareil présent qui n'y est plus est retiré :
    /// il revient en neuf. Un appareil absent qui n'y est plus est oublié.
    pub garde: Vec<Appareil>,
}
pub enum Accueil { Ouverte, Pleine, Fermee }

/// Une demande en cours. Elle existe tant que le jeu la liste.
pub struct Demande<'a> { pub id: DemandeId, pub quoi: QuoiDemande<'a> }
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DemandeId(pub u64);
pub enum QuoiDemande<'a> {
    Lire { fichier: FichierId, max: u64 },
    Image { fichier: FichierId, carre: u16, format: FormatImage },
    Rendu { scene: &'a Scene<Aucun>, echelle: u8, format: FormatImage },
    Telecharger { nom: &'a str, octets: &'a [u8] },
    Ecrire { flux: &'static str, ligne: &'a [u8] },
}
pub enum Reponse<'a> {
    Lu(Result<&'a [u8], Echec>),
    Image(Result<&'a [u8], Echec>),
    Rendu(Result<&'a [u8], Echec>),
    Fait(Result<(), Echec>),
}

/// Un refus. Il ne change rien : le moteur remet la mémoire.
pub enum Refus<A> {
    /// Montré au seul appareil qui a fait le geste, sans changer sa vue.
    Avis(A),
    /// Écarté sans un mot (une commande périmée de l'écran).
    Muet,
    /// Un secret manque ici : sur un appareil qui prédit, ce morceau attend la confirmation.
    /// Sur l'autorité, c'est une faute.
    Absent,
}
impl<A> From<Absent> for Refus<A> { /* … */ }

/// Ce qu'une étape sait, en lecture seule.
pub struct Contexte<'a> { /* caché */ }
impl Contexte<'_> {
    pub fn lecteur(&self) -> &Lecteur;
    pub fn ordre(&self) -> Ordre;        // l'ordre que prendra cette entrée si elle est acceptée
    pub fn maintenant(&self) -> Pas;
    pub fn hz(&self) -> Hz;
}
```

### 1.8 La déclaration

Chaque champ est obligatoire. `Declaration` n'a pas de `Default` : un champ ajouté demain casse la compilation de chaque jeu, là où il faut répondre. Une exemption est une valeur avec sa raison, jamais une absence (HIS-04, OUT-03, OUT-04).

```rust
pub struct Declaration {
    pub nom: &'static str,
    pub joueurs: Joueurs,
    pub appareils: Appareils,
    pub temps: Temps,
    pub pause: Pause,
    pub tenu: PolitiqueTenu,
    pub fin: Fin,
    pub prediction: Prediction,
    pub heritage: PolitiqueHeritage,
    pub souhaits: Souhaits,          // capacités souhaitées ; il n'existe pas de capacité requise
    pub proprietes: &'static [Propriete],
    pub exemptions: &'static [Exemption],
}
pub enum Joueurs { Entre { min: u16, max: u16 }, SeulEcran(Raison) }
pub struct Appareils { pub telephones: Admis, pub manettes: Admis, pub max: u16 }
pub enum Admis { Oui, Non(Raison) }
pub struct Temps {
    pub hz: Hz,
    pub rattrapage: u8,              // pas rattrapés au plus après un gel ; le reste est jeté
    pub fenetre: Duree,              // replacement des gestes en retard ; 0 = ordre d'arrivée
    pub age_max: Duree,              // un geste plus vieux est écarté, sauf un relâchement
}
pub enum Pause { Aucune(Raison), Jeu, Commun }
pub enum PolitiqueTenu {
    Aucun(Raison),
    Oui { bail: Bail, debit: u16 /* par seconde et par appareil */ },
}
pub enum Bail { Duree(Duree), Aucun(Raison) }
pub enum Fin { Oui, Non(Raison) }
pub enum Prediction { Aucune(Raison), Gestes }
pub enum PolitiqueHeritage { Oui, Non(Raison) }
pub struct Souhaits { pub vibration: Oui, pub mouvement: Oui, pub son: Oui, pub ecran_allume: Oui, pub micro: Oui, pub pad_natif: Oui }
pub enum Propriete {
    /// Une coupure plus courte que `moins_de`, sur n'importe quel appareil, ne change pas
    /// l'histoire définitive (`Resume` et vues), sous ces hypothèses (PUN-73, SQ-28).
    CoupureTransparente { moins_de: Duree, hypotheses: Raison },
    /// Les vues ne dépendent pas des identités : renuméroter les appareils permute les vues.
    Equivariance,
}
pub struct Exemption { pub item: &'static str, pub raison: Raison }

/// Les limites de l'autorité, connues des deux côtés.
pub struct Limites {
    pub geste: u32,       // octets d'un geste ou d'un tenu
    pub file: u16,        // événements en attente par appareil
    pub vue: u32,         // octets d'une vue encodée
    pub memoire: u32,     // pages de 64 Kio au plus
}
```

### 1.9 Les traits

Deux traits. `Jeu` porte la partie. `Epreuve` porte ce qui sert à l'éprouver. L'export exige les deux : un jeu sans bots ni invariants ne se compile pas en module.

```rust
pub trait Jeu: Projeter + Clone + Sized + 'static {
    type Geste: Message;   // événements de sa page et de ses boutons
    type Tenu: Message;    // `Aucun` si le jeu n'en a pas
    type Vue: Message;     // vue d'un téléphone ; `Aucun` sans téléphone
    type Avis: Message;    // ce que dit un refus
    type Heritage: Message;// ce qui passe à l'instance suivante et à la version suivante

    const LIMITES: Limites;
    fn declaration() -> Declaration;

    fn ouvrir(debut: Debut<Self::Heritage>) -> Self;
    fn etape(&mut self, ctx: &Contexte, e: Entree<'_, Self>) -> Result<(), Refus<Self::Avis>>;

    // Lectures. Le moteur remet la mémoire après chacune.
    fn ecran(&self, l: &Lecteur) -> Montre<Scene<Self::Geste>>;
    fn telephone(&self, l: &Lecteur) -> Montre<Self::Vue>;
    fn horloge(&self, l: &Lecteur) -> Horloge;
    fn salle(&self) -> Salle;
    fn demandes(&self) -> Vec<Demande<'_>>;
    fn heritage(&self) -> Option<Self::Heritage>;
    /// Les octets d'une image citée par une scène, que le commun n'a pas encore.
    fn ressource(&self, cle: Cle) -> Option<&[u8]>;
}

pub struct Debut<H> {
    pub graine: Graine,
    pub epoque: Utc,
    pub ecran: Appareil,
    pub heritage: Option<Herite<H>>,
}
pub enum Herite<H> { Lu(H), Illisible(Raison) }

pub trait Epreuve: Jeu {
    /// Ce que les invariants de transition comparent avant et après chaque entrée.
    type Resume: Message + PartialEq;
    /// Les pires cas de l'écran, un par variante. Le `match` exhaustif les exige tous.
    type Cas: Variantes;

    fn resume(&self) -> Self::Resume;
    fn invariants(&self, v: &mut Verif);
    fn transition(avant: &Self::Resume, apres: &Self::Resume, v: &mut Verif);

    /// Un téléphone simulé. Il ne voit que sa vue : il joue ce que l'écran demande.
    fn bot_telephone(vue: &Montre<Self::Vue>, h: &mut Hasard, style: Style) -> Vec<Coup<Self>>;
    /// L'organisateur simulé, à l'écran, qui lit l'état public.
    fn bot_ecran(&self, l: &Lecteur, h: &mut Hasard, style: Style) -> Vec<Coup<Self>>;

    fn pire_cas(cas: Self::Cas) -> Self;
}
pub enum Style { Fidele, Hostile }
pub struct Coup<J: Jeu> { pub apres: Duree, pub quoi: CoupQuoi<J> }
pub enum CoupQuoi<J: Jeu> { Geste(Action<J::Geste>), Tenu(Lache<J::Tenu>), Couper(Duree), Recharger }

/// Un invariant nommé. Le commun compte combien de fois chacun a été évalué.
pub struct Verif { /* caché */ }
impl Verif { pub fn exige(&mut self, nom: &'static str, tenu: Oui); }

pub trait Variantes: Sized + 'static { const TOUTES: &'static [Self]; }
pub use jeu_derive::Variantes;

/// Ce qui peut traverser la frontière : sérialisable, décodage strict, forme connue.
pub trait Message: Serialize + DeserializeOwned + Forme + 'static {}
pub use jeu_derive::Forme;   // empreinte de la forme, pour la version
```

### 1.10 L'export

```rust
// Dans le jeu, une ligne :
jeu::exporter!(Punchline);
```

La macro crée l'état statique et la fonction exportée. Son expansion porte le seul attribut `unsafe` du jeu (`#[unsafe(no_mangle)]`), avec `#[expect(unsafe_code, reason = "point d'entrée généré par jeu::exporter!")]`. `soiree check` vérifie que c'est le seul.

### 1.11 La copie native

```rust
pub mod natif {
    /// Le même contrat, sans WebAssembly : pour les tests rapides et les mutations.
    /// Un refus se remet par un clone pris avant l'appel.
    pub struct Partie<J: Epreuve> { /* … */ }
    impl<J: Epreuve> Partie<J> {
        pub fn ouvrir(graine: Graine) -> Self;
        pub fn entree(&mut self, e: EntreeNative<J>) -> Issue<J::Avis>;
        pub fn regard(&self, qui: Qui) -> RegardNatif<J>;
    }
}
```

La copie native n'a pas les garanties de l'enfermement. L'épreuve compare ses `Resume`, ses vues et ses demandes à celles du module, entrée par entrée.

---

## 2. Le contrat binaire

### 2.1 Ce que le module exporte et importe

```wat
(module
  (memory (export "memory") $min $max)        ;; $max obligatoire, = Limites.memoire
  (func (export "etape") (param $n i32) (result i64)))
```

- **Aucun import.** Le moteur refuse un module qui en a un. Constater ce à quoi le jeu a accès, c'est lire une section vide.
- **Deux exports**, `memory` et `etape`. Rien d'autre n'est appelé.
- **Pas de fonction de départ.** Rien ne tourne avant le premier appel.
- **Fonctionnalités admises :** WebAssembly 1.0, globales mutables, extension de signe, conversions sans piège, mémoire en bloc, valeurs multiples, types référence, SIMD 128. **Refusées :** fils, atomiques, mémoire partagée, SIMD relâché, mémoire 64 bits, exceptions, plusieurs mémoires, et toute instruction qui modifie une table.
- Cible : `wasm32-unknown-unknown`, `panic = "abort"`, contrôle de débordement actif en release.

### 2.2 Un appel

1. Le moteur écrit `n` octets dans la **boîte d'entrée** du module.
2. Il appelle `etape(n)`.
3. Le module rend `(longueur << 32) | adresse` : la sortie est dans sa mémoire, valable jusqu'à l'appel suivant.
4. Le moteur lit la sortie, la décode strictement, puis garde ou remet la mémoire (section 3).

`etape(0)` est **Bonjour** : un appel sans entrée. Aucun message encodé n'a zéro octet, il n'y a donc pas d'ambiguïté. La réponse de Bonjour donne :

```rust
// caché : jeu::abi
pub struct Bonjour<'a> {
    pub abi: u32,                    // version de l'enveloppe ; autre valeur : refus
    pub boite: (u32, u32),           // adresse et capacité de la boîte d'entrée
    pub sortie_max: u32,
    pub formes: [u8; 32],            // empreinte des formes : enveloppe + types du jeu
    pub declaration: Declaration,
    pub limites: Limites,
}
```

La boîte est un tableau statique du module, de taille fixe. Elle a la même adresse dans toutes les instances du même module. Il n'y a pas de fonction d'allocation exportée : la taille d'une entrée est bornée par les limites, et le moteur refuse au-dessus avant d'écrire.

### 2.3 L'enveloppe

Tous les appels passent par un seul type, `Appel`. Toutes les sorties par un seul type, `Retour`. Les types du jeu y voyagent en octets : le moteur ne les connaît pas, le module les décode lui-même, dans la colle de `jeu`, avant d'appeler le code du jeu.

```rust
// caché : jeu::abi
pub enum Appel<'a> {
    Ouvrir { graine: Graine, epoque: Utc, ecran: Appareil, heritage: Option<&'a [u8]> },
    Entree { ctx: CtxBrut, entree: EntreeBrute<'a> },          // gardée si acceptée
    Regarder {                                                  // toujours remise
        maintenant: Pas, definitif: Pas, ordre: Ordre,
        lecteurs: &'a [QuiBrut],
        demandes_connues: &'a [(DemandeId, [u8; 16])],
        ressources: &'a [Cle],
    },
    Resumer,
    Verifier,
    Controler { qui: QuiBrut },                                 // vue directe = vue sur projection ?
    Bot { qui: QuiBrut, graine: u64, style: Style },
    PireCas { indice: u32 },
    Heriter,
    Projeter { qui: QuiBrut },                                  // pour la prédiction
    Charger { projection: &'a [u8] },                           // côté appareil, gardé
}

pub enum Retour<'a> {
    Bonjour(Bonjour<'a>),
    Ouvert,
    Accepte,
    Refuse { avis: Option<&'a [u8]> },                          // None : muet
    Absent,
    Illisible(ErreurDecodage),                                  // la frontière a refusé
    Regard {
        horloge: Horloge, salle: Salle,
        vues: Vec<(QuiBrut, &'a [u8])>,                          // Montre<…> encodé
        demandes: Vec<DemandeBrute<'a>>,                         // seulement les nouvelles
        ressources: Vec<(Cle, &'a [u8])>,
        vivant: u64,                                             // octets vivants de l'allocateur
    },
    Resume(&'a [u8]),
    Verif(Vec<(&'static str, Oui)>),
    Controle(Oui),
    Bot(&'a [u8]),
    PireCas { nom: &'static str, etat: Oui },
    Heritage(Option<&'a [u8]>),
    Projection(&'a [u8]),
}
```

### 2.4 La sérialisation

- `postcard` 1.x des deux côtés, avec les mêmes types Rust : l'enveloppe est définie une fois, dans `jeu`. Les types d'un jeu sont définis une fois, dans le jeu, et sa page les utilise.
- **Décodage strict :** un octet restant est un refus (OUT-146, REM-59). Une longueur plus grande que ce qui reste échoue avant toute réservation (REM-58).
- **Les flottants** ne voyagent que dans `Fini`, `Axe` et les types de `dessin`, qui refusent NaN, l'infini et les sous-normaux au décodage.
- **La forme :** `#[derive(Forme)]` calcule une empreinte de la forme de chaque type. `formes` couvre l'enveloppe et les cinq types du jeu. Une page d'une autre forme est refusée et dit « recharge la page » (SAL-41, PUN-44, REM-77).

### 2.5 Les versions

| Identité | D'où elle vient | Ce qu'elle protège |
|---|---|---|
| `abi` | une constante de `jeu`, montée à chaque changement de l'enveloppe | moteur et module parlent la même enveloppe |
| `formes` | calculée depuis les types | page et module décodent les mêmes octets |
| empreinte du module | SHA-256 des octets du module livré, avant réécriture | la version du jeu ; elle nomme le journal, les instantanés, les rapports (OUT-37) |
| empreinte de la réécriture | version des passes du moteur | deux moteurs comparent le même module réécrit |

Un instantané de mémoire n'a de sens que pour la même empreinte de module. D'une version à l'autre, seul l'héritage passe (section 3.6).

### 2.6 Ce qui a été retiré

| Retiré | Ce qui le remplace |
|---|---|
| Tout import (heure, hasard, journal, allocation de l'hôte) | le temps et la graine sont des entrées ; l'allocateur est dans le module |
| Une fonction par usage (vue, bots, invariants…) | une seule porte ; une lecture est un appel remis |
| Une fonction d'allocation exportée | une boîte statique de taille déclarée |
| Les sorties de l'étape (effets, demandes, commandes à la salle) | l'étape rend « accepté » ou « refusé » ; le reste se lit dans l'état |
| Une file d'effets à émettre | un effet est une ligne de la vue, avec identité et instant |
| Une API d'annulation des demandes | une demande retirée de l'état est annulée ; sa réponse n'entre plus |
| Des minuteurs | `Reveil` : un pas à l'échéance, ou à chaque pas, ou jamais |
| Retirer, oublier, fermer : trois commandes | `Salle` : qui la salle garde, et si elle accueille |
| Une sérialisation de l'état écrite ou dérivée | la mémoire est l'état ; seul l'héritage est sérialisé, et seulement s'il existe |
| Le numéro de phase dans chaque geste du jeu | `vu`, l'ordre de la vue, mis par le commun |
| Les capacités requises | il n'existe pas de champ pour en requérir une |
| `std` | `no_std` : quatre règles d'écriture deviennent des formes |

Ce qui reste et ne peut pas partir : la déclaration, le lecteur, `Secret`, la projection dérivée, l'enveloppe.

---

## 3. Le moteur

### 3.1 La réécriture au chargement

Le moteur valide le module, puis le réécrit. Les passes sont du code du commun, versionné, et chacune a un test qui rougit quand on la retire.

1. **Carburant.** Une globale `i64` décrémentée à l'entrée de chaque bloc, du coût du bloc. Négative : piège. Le compte est en instructions, donc le même sur tous les moteurs : une faute de budget se rejoue à l'identique.
2. **Profondeur.** Un compteur d'appels, plafonné. La pile native diffère d'un moteur à l'autre ; ce plafond rend le dépassement déterministe.
3. **NaN canoniques.** Après chaque opération flottante qui peut produire un NaN, un NaN devient le NaN canonique (SQ-68).
4. **Pages sales.** Avant chaque écriture en mémoire, et sur toute la plage d'un `memory.copy` ou `memory.fill` : si le bloc de 4 Kio n'est pas encore marqué, il est copié dans un journal d'annulation, puis marqué. Le journal et la carte vivent dans une seconde mémoire ajoutée par le moteur, que le jeu ne peut pas nommer.
5. **Exports ajoutés** pour le moteur seul : la globale de carburant, les globales mutables, la mémoire d'annulation, deux fonctions générées (`valider`, `remettre`).

Le module réécrit tourne pareil dans un navigateur et dans un moteur natif : budget, NaN, profondeur et pages sales sont dans son code, pas dans le moteur qui l'exécute.

### 3.2 Appeler, garder, remettre

```rust
// commun : jeu-moteur
fn appeler(&mut self, appel: &Appel, budget: Carburant) -> Result<Sortie, Faute> {
    self.module.ecrire_boite(&postcard::to_slice(appel)?)?;      // refus si > capacité
    self.module.set_carburant(budget);
    let brut = match self.module.etape(n) {
        Ok(r) => r,
        Err(piege) => { self.module.remettre(); return Err(Faute::depuis(piege)); }
    };
    let sortie = match decoder_strict(self.module.lire(brut)?) {
        Ok(s) => s,
        Err(e) => { self.module.remettre(); return Err(Faute::SortieIllisible(e)); }
    };
    if garder(appel, &sortie) { self.module.valider() } else { self.module.remettre() }
    Ok(sortie)
}

fn garder(appel: &Appel, s: &Retour) -> bool {
    matches!((appel, s), (Appel::Ouvrir { .. }, Retour::Ouvert)
                       | (Appel::Entree { .. }, Retour::Accepte)
                       | (Appel::Charger { .. }, Retour::Ouvert))
}
```

`valider` vide la carte des blocs marqués. `remettre` recopie les blocs du journal et remet les globales. Les deux coûtent ce qui a été écrit, pas la taille de la mémoire. Un état de dizaines de Mo (tier list) ne coûte à remettre que ses quelques blocs touchés.

### 3.3 La boucle de l'autorité

L'autorité tourne dans un Worker, sur l'ordinateur de la soirée.

```
à chaque image :
  entrées ← transport.prendre()                   // ordonnées, dédoublonnées, validées
  si l'horloge l'exige : entrées += Pas(p) pour chaque pas dû, au plus `rattrapage`
  pour chaque entrée, dans l'ordre fixé :
      ctx ← { ordre, maintenant, lecteur: Autorite }
      issue ← appeler(Entree { ctx, entrée })
      journal.ajouter(entrée, issue)              // gardée ou non, pour le diagnostic
      si Refuse { avis } : transport.avis(entrée.de, avis)
      si Faute : isoler(entrée, faute)
  regard ← appeler(Regarder { lecteurs présents, demandes connues, … })   // une fois par image
  pour chaque appareil : si octets(vue) ≠ dernière envoyée : envoyer(vue, ordre)
  demandes : lancer les nouvelles, annuler celles qui ont disparu
  salle : retirer les présents qui ne sont plus gardés, oublier les absents non gardés
  horloge : programmer le prochain pas selon `reveil`
  instantané si l'intervalle est écoulé
```

**L'ordre fixé.** Avec une fenêtre de 0, les entrées passent dans l'ordre d'arrivée à l'autorité. Avec une fenêtre, dans l'ordre de `fait`. Deux gestes du même pas se départagent par un tirage sur la graine, jamais par l'identité d'un appareil (CAT-27).

**Le replacement.** Un geste en retard, dans la fenêtre : le moteur remet la mémoire à l'instantané du pas visé, insère le geste, rejoue la suite. Les instantanés de la fenêtre sont des journaux d'annulation empilés, un par pas. Au-delà de la fenêtre, le pas est définitif, et `Lecteur::definitif` avance.

**Le temps.** `Reveil::A(p)` donne un seul pas, à `p` ou au réveil s'il est passé : un gel de dix minutes ne traverse qu'une phase, et la suivante part du pas courant (PUN-34). `ChaquePas` donne au plus `rattrapage` pas d'affilée après un gel ; le reste est jeté, et le numéro saute. Avec `Pause::Commun`, le moteur ne donne plus de pas et relâche les tenus.

**Les tenus.** Le moteur ne livre qu'un tenu par appareil et par pas, le plus récent ; il écarte un numéro plus petit que le dernier livré (REM-13, REM-14). Il livre `Lache(Silence)` quand le bail déclaré passe sans nouvelle (SQ-01), et écarte un tenu plus vieux que `age_max`, sauf un relâchement (SQ-02).

### 3.4 Couper un pas trop long

- Le carburant épuisé est un piège. Le moteur remet la mémoire : l'entrée n'a rien changé.
- Le budget dépend de l'appel : un pour une entrée, un pour un regard, un plus large pour `Ouvrir` et `Heriter`. Les valeurs sont **à mesurer** sur les trois jeux, puis écrites dans `CONTRAT.md` avec leur date.
- Un second garde-fou, en temps réel : si un appel dépasse une durée (bug du moteur, carburant mal compté), le Worker est arrêté, et l'autorité repart de son dernier instantané et du journal (3.5). C'est une **panne de l'outil**, jamais une faute du jeu (HIS-06, OUT-11).

### 3.5 Reprendre après un plantage

**Une entrée fautive** (piège, carburant, profondeur, mémoire au-delà du maximum, sortie illisible, `Absent` sur l'autorité) :
- la mémoire est remise ;
- l'entrée est écartée pour toujours : la reprise ne la rejoue pas ;
- le rapport porte la faute, l'entrée isolée, la graine, l'empreinte de l'état d'avant et la position dans le journal. Il suffit pour rejouer l'échec (HIS-07) ;
- l'appareil qui a fait le geste reçoit un avis du commun : « Ce geste n'a pas pu être pris en compte. » ;
- un `Pas` fautif est écarté ; le suivant porte le numéro suivant. Après trois pas fautifs de suite, le moteur arrête de donner des pas et l'écran dit « Le jeu est en panne ». Aucune forme ne sauve un jeu qui plante à chaque pas : c'est à l'épreuve de le trouver avant la soirée ;
- un `Regarder` fautif garde la dernière vue valide de chaque appareil. Aucun appareil ne reste sans vue.

**Un onglet de l'autorité qui se recharge ou meurt** :
- le moteur garde, hors de l'onglet, un instantané (mémoire, globales, nombre de pages) et le journal des entrées qui le suivent, chiffrés. Un instantané complet par intervalle, et entre deux, seulement les blocs écrits ;
- au rechargement : même empreinte de module, instanciation, restauration, rejeu du journal, puis un regard ; chaque appareil reçoit sa vue à sa reconnexion ;
- une empreinte différente (le jeu a changé) : `Ouvrir` avec l'héritage, si le jeu en a un.

**Ce qui ne quitte jamais l'ordinateur de la soirée en clair :** la mémoire. Une mémoire libérée peut encore contenir un secret. L'allocateur de `jeu` remet à zéro ce qu'il libère, mais seul le chiffrement fait la garantie.

### 3.6 L'héritage

Un seul mécanisme pour trois besoins :
- **rejouer** dans une instance neuve, avec les mêmes joueurs : le moteur instancie, appelle `Ouvrir` avec l'héritage de l'ancienne, puis redonne `Arrive` pour chaque appareil présent, avec le même identifiant ;
- **changer de version** : l'héritage est un enum versionné par le jeu (`V1`, `V2`…), converti explicitement. Son discriminant se lit avant sa forme (TL-13) ;
- **une sauvegarde abîmée** : `Herite::Illisible(raison)`. Le jeu part d'un état valide et le dit ; le commun garde les octets illisibles sans les écraser (HIS-51, TL-67).

Un jeu sans héritage le déclare, avec sa raison.

### 3.7 Comparer deux exécutions

- **Module contre module** (deux moteurs, deux navigateurs, deux passages) : après chaque entrée gardée, le moteur calcule une empreinte de la mémoire en arbre, bloc par bloc. Seuls les blocs écrits sont rehachés (TL-22). La première entrée où les empreintes divergent est rapportée.
- **Module contre copie native** : les mémoires diffèrent par nature. On compare, entrée par entrée, l'issue, le `Resume`, les vues encodées, l'horloge, la salle et les demandes.
- L'empreinte couvre tout : générateur, historique d'annulation, compteurs (TL-21, SAL-158). Rien n'est oublié, puisque rien n'est choisi.

---

## 4. Secrets, projection et prédiction

### 4.1 Sur l'autorité

- L'étape lit tout : son `Contexte` porte le lecteur `Autorite`.
- La vue d'un téléphone se calcule avec `Qui::Appareil(id)`. La vue de l'écran avec `Qui::Public` : la télé est dessinée sur la machine qui détient les secrets, mais elle ne les lit pas (CAT-46).
- Un appareil ne reçoit que sa vue, jamais l'état. Une vue ne peut pas contenir de `Secret` (1.6).
- Le jeu peut encore recopier un secret dans un champ public trop tôt. C'est une règle de jeu, que ses invariants doivent tenir. Le commun, lui, vérifie la forme : `Controler` calcule la vue sur l'état, puis sur `projeter(état, lecteur)`, et compare les octets. Différents : la vue a lu ce qu'elle n'avait pas le droit de lire. L'épreuve le fait à chaque pas, pour chaque lecteur.

### 4.2 Sur un appareil qui prédit

Seulement si la déclaration dit `Prediction::Gestes`. Aucun des trois jeux ne prédit.

- L'appareil reçoit en plus sa projection : `Projeter { qui }` sur l'autorité, puis `Charger` dans sa propre instance du même module. La projection est le seul endroit où l'état est sérialisé, et seulement pour ces jeux-là.
- Il applique ses propres gestes déclarés prédictibles à son instance, et montre la vue qui en sort.
- Une étape qui lit un secret absent rend `Refus::Absent`. Le moteur de l'appareil remet sa mémoire : ce geste attend la confirmation.
- À chaque projection confirmée : `Charger`, puis rejouer les gestes encore en attente.
- Couper la prédiction, c'est ne plus charger de projection : l'appareil revient aux vues de l'autorité. C'est la voie de secours.
- Seuls les effets de la vue prédite qui viennent du geste de l'appareil se jouent avant confirmation. Un effet prédit à tort disparaît de la vue suivante : il est annulé.

### 4.3 Ce qui ne s'annonce qu'une fois définitif

Le lecteur porte `definitif`. Un jeu qui garde l'instant d'un résultat (`marque_a: Pas`) ne le montre que si `marque_a <= l.definitif()`. Avec une fenêtre de 0, `definitif` est le pas courant, et il n'y a rien à faire.

---

## 5. Vues, effets et demandes

### 5.1 Deux sortes de vues

| Appareil | Type de vue | Qui la dessine |
|---|---|---|
| Écran | `Montre<Scene<Self::Geste>>` | le commun, sur le GPU, dans la zone sûre |
| Téléphone | `Montre<Self::Vue>` | la page du jeu, dans le navigateur |

Le moteur demande les vues une fois par image, en un seul `Regarder`, pour les lecteurs présents. Il n'envoie une vue que si ses octets ont changé. Chaque vue part avec l'ordre de l'état qu'elle montre ; la page le renvoie dans chaque geste (`vu`).

### 5.2 La scène

`dessin` est une crate du commun, hors du noyau. Le noyau n'en connaît que des octets. Ce dont les cahiers ont besoin, et que la scène doit porter :
- des nœuds avec une **clé stable**, pour glisser entre deux pas sans saut (Sidequest) et garder un champ en cours d'édition (TL-33) ;
- des longueurs en unités de scène, une zone sûre, des textes qui s'ajustent à leur boîte parmi des tailles données (Punchline, export de la tier list) ;
- des images citées par une `Cle`, jamais une URL ; le commun demande les octets manquants par `ressource` (TL-15) ;
- des boutons qui portent un `G` : un clic devient `Action::Jeu(g)` depuis `Role::Ecran` ;
- des composants : champ de texte (`Saisie`), éléments déplaçables (`Deplace`), sélecteur de fichiers, lien de téléchargement, fenêtre sur un voile, défilement, info-bulles et noms accessibles. Leurs entrées sont typées par le commun et validées contre la scène affichée.

Les entrées des composants :

```rust
pub struct Saisie { pub champ: Cle, pub texte: String /* borné par la limite du champ */, pub fin: FinSaisie }
pub enum FinSaisie { EnCours, Entree, PerteDeFocus }    // jamais pendant une composition (TL-32)
pub enum Deplace {
    Survol { objet: Cle, zone: Cle, indice: u32 },       // compté dans l'ordre de la zone, sans l'objet tenu (TL-39)
    Depose { objet: Cle, zone: Cle, indice: u32 },
    Annule { objet: Cle },                                // Échap, pointercancel
}
pub struct Fichiers { pub geste: u64, pub liste: Vec<FichierRecu> }
pub struct FichierRecu { pub id: FichierId, pub nom: Texte<255, Graphemes>, pub genre: Texte<127, Utf16>, pub taille: u64 }
pub struct Touche { pub code: Code, pub modifs: Modifs }
pub struct Pointeur { pub id: u32, pub genre: GenrePointeur, pub bouton: u8, pub phase: PhasePointeur, pub pos: (Fini, Fini) }
```

Le défilement automatique près du bord est fait par le composant, en vitesse par seconde et avec un plafond au réveil (TL-41, TL-42). Le jeu n'a pas besoin de pas pour cela.

### 5.3 Les effets

Un effet est une ligne de la vue : `Effet { id, a, quoi }`. Le commun de chaque appareil garde les identités déjà jouées :
- un réaffichage, une reconnexion ou un rejeu revoit la même identité : rien ne rejoue (PUN-66) ;
- un effet vu après son instant, au-delà d'une tolérance (250 ms dans Punchline), ne se joue pas : un appareil absent ne le rejoue pas en retard ;
- un effet futur se programme à son instant, en compensant le retard de l'appareil ;
- un effet qui disparaît de la vue avant son instant est annulé.

L'identité vient de l'état : par exemple le numéro de la phase. Elle ne dépend pas d'un compteur de rendu.

### 5.4 Les demandes

Une demande existe tant que le jeu la liste dans `demandes()`. Le moteur compare la liste d'une image à l'autre :
- une identité nouvelle : il la lance ;
- une identité qui disparaît : il l'annule ;
- une réponse n'entre que si son identité est encore listée. Sinon elle est jetée sans atteindre le jeu (TL-45) ;
- un remplacement de tout l'état (Reset, restauration) retire les demandes de l'état : elles sont toutes annulées (TL-44, TL-46).

L'identité d'une demande est son `DemandeId` et une empreinte de son contenu : une annulation qui redonne un identifiant déjà servi, avec un autre contenu, fait une demande neuve (TL-23). Le module n'encode que les demandes que le moteur ne connaît pas encore (`demandes_connues`). Le nombre de demandes listées est borné par une limite ; l'épreuve vérifie que le jeu retire ce qui est fini.

Les fichiers reçus restent chez le commun. Le jeu ne reçoit que leur nom, leur type et leur taille. Il lit ce qu'il veut par `Lire { max }`, ou fait transformer une image par `Image` : l'original n'entre jamais dans l'état (TL-14, TL-60).

### 5.5 Ce qui reste au commun, hors du noyau

Le noyau ne porte aucune de ces choses ; elles se construisent au-dessus, sans que le jeu change :
- le lien et son seul message d'état, qui distingue « jamais atteint », « perdu », « page qui ne répond pas » et « permission refusée » ;
- le battement applicatif, le repli de canal, les numéros par appareil, la file qui survit aux coupures, l'au-revoir (SAL-43, SAL-56, SAL-58, SAL-103, SAL-124) ;
- les refus d'accueil (salle pleine, fermée, autre version, faux rôle), et « reprendre ici » pour un onglet remplacé (SAL-18, SAL-42) ;
- la durée de vie des ressources d'une page, et la fin de chaque geste en cours quand la vue change (REM-40) ;
- le compte à rebours sur l'appareil, depuis l'échéance de la vue et un pas estimé qui ne recule jamais ;
- la conservation des tenus pendant une coupure, et leur relâchement au départ du premier plan (REM-10, REM-11) ;
- le compteur de présentations du rendu (OUT-33), le panneau de diagnostic, la marque dans le journal (une touche réservée par le commun) ;
- les avertissements permanents de stockage, à côté de l'avis du jeu, sans que l'un cache l'autre (TL-71).

La page téléphone d'un jeu reçoit `Montre<Vue>` et l'ordre, et envoie par `page::envoyer(geste) -> Result<(), Envoi>` et `page::tenir(valeur)`. `Envoi` dit `TropGros` ou `FilePleine` : jamais une perte silencieuse (SAL-07, OUT-197).

---

## 6. Bots, invariants et épreuve

### 6.1 Ce que le jeu fournit

- **Deux bots.** `bot_telephone` est une fonction de la vue seule : il ne peut jouer que ce que l'écran montre. `bot_ecran` joue l'organisateur, sur l'état public. Chacun rend des coups datés (« voter après 1,5 s »), en style fidèle ou hostile.
- **Des invariants d'état** (`invariants`), nommés, évalués après chaque entrée gardée.
- **Des invariants de transition** (`transition`), sur un `Resume` pris avant et après. Exemple : une relance ne retire jamais un point (SQ-29).
- **Un pire cas par variante d'écran** (`pire_cas`), que le `match` exhaustif rend obligatoire.
- **Des propriétés déclarées**, avec leurs hypothèses : coupure transparente, équivariance.

Tout est du code. Le commun peut le lancer et le muter (HIS-02, HIS-03).

### 6.2 Comment l'épreuve s'en sert

1. **Constater.** Section d'imports vide, deux exports, fonctionnalités admises. Bonjour, puis validation de la déclaration : raisons non vides, `min <= max`, limites cohérentes. Une déclaration incohérente est refusée dès là (OUT-200).
2. **Ouvrir une soirée simulée**, sur une horloge simulée, à vitesse machine (OUT-83) : autant de téléphones simulés que `joueurs.max` (HIS-13), un organisateur simulé.
3. **À chaque entrée gardée :**
   - les invariants et la transition ;
   - **jamais figée** (CAT-78) : pour chaque appareil de `Attend::Appareils`, l'épreuve demande ses coups au bot, essaie chacun (les essais sont remis), et exige qu'un coup au moins soit accepté, ou que `echeance` existe et fasse avancer `Phase::numero` ;
   - **secrets** : `Controler` pour chaque lecteur ;
   - **refus** : après une entrée refusée, l'empreinte de la mémoire doit être celle d'avant. C'est le contrôle du mécanisme lui-même ; une passe « remettre » retirée doit le faire rougir (OUT-26, OUT-27) ;
   - **progression** : `Phase::numero` avance, et une partie qui déclare `Fin::Oui` atteint `fin: Oui`, puis repart (OUT-29).
4. **Pannes** injectées sur le transport simulé, constatées avant d'être jugées : coupure, silence sans fermeture, trou noir, paquet retenu puis livré, rechargement d'un téléphone, onglet jumeau, redémarrage de l'autorité depuis instantané et journal (OUT-26, OUT-75, OUT-77, OUT-79). Les pannes se mêlent, comme dans une soirée.
5. **Propriétés.** Même soirée avec et sans coupures courtes, histoires comparées (PUN-73). Même soirée avec des identités tirées autrement, vues comparées (CAT-27). Avec et sans prédiction, histoires définitives comparées.
6. **Entrées absurdes.** Octets au hasard dans chaque champ de l'enveloppe : `Illisible`, jamais une faute. Coups hostiles des bots : textes hostiles, fichiers abîmés, gestes périmés.
7. **Déterminisme.** Chaque soirée deux fois, et dans deux moteurs ; la copie native contre le module.
8. **Mémoire.** `vivant` sur une soirée longue : la pente, pas un instantané (HIS-10, OUT-84).
9. **Pires cas.** Chaque scène de `pire_cas` rendue sous plusieurs formats ; débordements, sorties d'écran, chevauchements mesurés dans `dessin`.
10. **Mutations** de la copie native : les contrôles ci-dessus doivent tuer les mutants ; le rapport est sans seuil (OUT-95, OUT-96).

Le rapport distingue : réussi, échec du jeu (avec graine et entrées pour le rejouer), panne de l'outil, sans objet (avec la raison déclarée), pas exécuté. Un invariant jamais évalué est « pas exécuté », jamais réussi (OUT-01, OUT-07). Une attente qui expire dit sa dernière valeur, sa durée et le nombre d'images vues (OUT-94).

---

## 7. Les trois jeux, en esquisse

### 7.1 Punchline

```rust
#![no_std]
extern crate alloc;
use jeu::*;

pub const PRENOM: u32 = 12;
pub const REPONSE: u32 = 80;
type Prenom = Texte<PRENOM, Capitales>;
type Case = Texte<REPONSE, Capitales>;

#[derive(Clone, Projeter)]
pub struct Punchline {
    hasard: Hasard,
    ecran: Appareil,
    phase: Phase_,                       // Salon, Depart{fin}, Annonce{fin}, Ecriture{fin}, …
    debut: Ordre,                        // ordre d'entrée dans la phase : refuse le périmé
    debut_pas: Pas,
    numero_phase: u64,
    joueurs: Vec<Joueur>,                // ordre d'arrivée, robots compris
    telephones: BTreeMap<Appareil, Telephone>,
    manche: u8,
    paquets: Paquets,
    duels: Vec<Duel>,
    brouillons: BTreeMap<(JoueurId, ConsigneId), Secret<Vec<Case>>>,
}
#[derive(Clone, Projeter)]
struct Duel {
    consigne: Consigne,
    cotes: [JoueurId; 2],
    reponses: [Secret<Option<Reponse>>; 2],     // pour l'auteur seul pendant l'écriture
    affichees: Option<[Reponse; 2]>,            // copiées au début du vote, sans auteur
    votes: BTreeMap<JoueurId, Secret<Cote>>,    // pour l'autorité seule
    revelation: Option<Revelation>,             // copiée au début de la révélation
}
#[derive(Clone, Projeter)]
struct Telephone { joueur: Option<JoueurId>, present: Presence_, parti_a: Option<Pas>, revenu_a: Option<Pas> }

#[derive(Serialize, Deserialize, Forme)]
pub enum Geste { Prenom(Prenom), Personnage(u8), Lancer, Annuler, Envoyer { consigne: ConsigneId, cases: Vec<Case> }, JeSeche { consigne: ConsigneId }, Voter(Cote), Rejouer }
#[derive(Serialize, Deserialize, Forme)]
pub struct Brouillon { pub consigne: ConsigneId, pub cases: Vec<Case> }

impl Jeu for Punchline {
    type Geste = Geste;
    type Tenu = Brouillon;
    type Vue = VuePunchline;
    type Avis = Avis;                        // « Trop tard : l'écran a changé. », …
    type Heritage = Aucun;

    const LIMITES: Limites = Limites { geste: 4096, file: 64, vue: 16 * 1024, memoire: 256 };

    fn declaration() -> Declaration {
        Declaration {
            nom: "punchline",
            joueurs: Joueurs::Entre { min: 3, max: 8 },
            appareils: Appareils { telephones: Admis::Oui, manettes: Admis::Non(raison!("aucune manette n'a de rôle")), max: 24 },
            temps: Temps { hz: Hz(60), rattrapage: 1, fenetre: Duree::ms(0), age_max: Duree::s(2) },
            pause: Pause::Aucune(raison!("une partie finit toujours : chaque phase a une échéance")),
            tenu: PolitiqueTenu::Oui {
                bail: Bail::Aucun(raison!("un brouillon reste valable quand le téléphone se tait (PUN-02)")),
                debit: 10,
            },
            fin: Fin::Oui,
            prediction: Prediction::Aucune(raison!("tour par tour : un échange toutes les quelques secondes suffit")),
            heritage: PolitiqueHeritage::Non(raison!("une partie ne survit pas à un changement de version")),
            souhaits: Souhaits { ecran_allume: Oui::Oui, vibration: Oui::Non, mouvement: Oui::Non, son: Oui::Non, micro: Oui::Non, pad_natif: Oui::Non },
            proprietes: &[Propriete::CoupureTransparente {
                moins_de: Duree::s(20),
                hypotheses: raison!("le téléphone coupé avait déjà envoyé ce qu'il avait tapé"),
            }],
            exemptions: &[],
        }
    }

    fn ouvrir(d: Debut<Aucun>) -> Self { Punchline::salon(Hasard::depuis(&d.graine, 0), d.ecran) }

    fn etape(&mut self, ctx: &Contexte, e: Entree<'_, Self>) -> Result<(), Refus<Avis>> {
        match e {
            Entree::Pas(p) => self.temps(ctx, p),          // robots à +1,5 s, grâces de 20 s et 90 s, échéances
            Entree::Presence(pr) => self.presence(ctx, pr),
            Entree::Tenu(t) => self.brouillon(ctx, t),      // compte si `t.recu` <= fin de l'écriture
            Entree::Reponse(..) => Err(Refus::Muet),        // ne demande rien
            Entree::Geste(g) => {
                if g.vu < self.debut {                       // PUN-38
                    return Err(match g.role { Role::Ecran => Refus::Muet, Role::Telephone | Role::Manette => Refus::Avis(Avis::TropTard) });
                }
                match (g.role, g.quoi) {
                    (Role::Ecran, Action::Touche(t)) => self.organiser(ctx, t),     // Espace, Entrée, Échap, B
                    (Role::Ecran, Action::Jeu(c)) => self.bouton_ecran(ctx, c),
                    (Role::Telephone, Action::Jeu(c)) => self.joueur(ctx, g.de, c),
                    (Role::Telephone | Role::Manette | Role::Ecran, Action::Touche(_) | Action::Pointeur(_) | Action::Saisie(_)
                        | Action::Deplace(_) | Action::Fichiers(_) | Action::Jeu(_)) => Err(Refus::Avis(Avis::PasMaintenant)),
                }
            }
        }
    }

    fn ecran(&self, l: &Lecteur) -> Montre<Scene<Geste>> {
        Montre {
            vue: scene::dessiner(self, l),                  // lit les réponses seulement par `affichees`
            effets: vec![Effet { id: EffetId(self.numero_phase), a: self.debut_pas, quoi: QuoiEffet::Son(Son::Signal(self.phase.signal())) }],
            reglages: Reglages::ECRAN,
        }
    }
    fn telephone(&self, l: &Lecteur) -> Montre<VuePunchline> { /* sa consigne, ses brouillons lus par `l` */ }
    fn horloge(&self, _: &Lecteur) -> Horloge {
        Horloge {
            reveil: Reveil::A(self.prochain_reveil()),     // min(échéance, robots, fins de grâce)
            attend: match &self.phase {
                Phase_::Ecriture { fin } => Attend::Appareils { qui: self.attendus_ecriture(), echeance: Some(*fin) },
                Phase_::Vote { fin, .. } => Attend::Appareils { qui: self.votants_attendus(), echeance: Some(*fin) },
                Phase_::Salon | Phase_::Fin => Attend::Appareils { qui: self.organisateurs(), echeance: None },
                Phase_::Depart { fin } | Phase_::Annonce { fin } | Phase_::Revelation { fin, .. } | Phase_::Scores { fin } =>
                    Attend::Appareils { qui: vec![], echeance: Some(*fin) },
            },
            phase: Phase { nom: self.phase.nom(), numero: self.numero_phase, fin: self.phase.est_fin() },
        }
    }
    fn salle(&self) -> Salle {
        Salle {
            accueil: Accueil::Ouverte,                      // le refus « partie en cours » est un avis du jeu
            garde: self.telephones.keys().copied().collect(), // la grâce de 90 s est une règle de l'étape
        }
    }
    fn demandes(&self) -> Vec<Demande<'_>> { vec![] }
    fn heritage(&self) -> Option<Aucun> { None }
    fn ressource(&self, _: Cle) -> Option<&[u8]> { None }
}

impl Epreuve for Punchline {
    type Resume = Scores;                                   // points par joueur, numéro de phase
    type Cas = CasEcran;                                    // Salon8, Ecriture, VoteFinaleLong, Jinx, Forfait, Egalite8, Fin3…

    fn invariants(&self, v: &mut Verif) {
        v.exige("score au plus 13 200", self.joueurs.iter().all(|j| j.score <= 13_200).into());
        v.exige("chaque joueur a deux consignes", self.consignes_par_joueur().all(|n| n == 2 || self.phase.est_finale()).into());
        v.exige("votes reçus <= votes possibles", (self.votes_recus() <= self.votes_possibles()).into());   // PUN-17
    }
    fn transition(a: &Scores, b: &Scores, v: &mut Verif) {
        v.exige("un score ne baisse que par « On rejoue ! »", (b.relance > a.relance || b.points.iter().zip(&a.points).all(|(x, y)| x >= y)).into());
    }
    fn bot_telephone(m: &Montre<VuePunchline>, h: &mut Hasard, s: Style) -> Vec<Coup<Self>> {
        match &m.vue {
            VuePunchline::Prenom { .. } => vec![Coup::geste(Duree::ms(300), Geste::Prenom(prenoms::tirer(h, s)))],
            VuePunchline::Ecriture { consigne, cases, .. } => vec![
                Coup::tenu(Duree::ms(800), Brouillon { consigne: *consigne, cases: textes::tirer(h, s, *cases) }),
                Coup::geste(Duree::ms(1500), Geste::Envoyer { consigne: *consigne, cases: textes::tirer(h, s, *cases) }),
            ],
            VuePunchline::Vote { .. } => vec![Coup::geste(Duree::ms(1500), Geste::Voter(Cote::tirer(h)))],
            VuePunchline::Salon { vip: Oui::Oui, prets: Oui::Oui, .. } | VuePunchline::Fin { vip: Oui::Oui, .. } =>
                vec![Coup::geste(Duree::s(2), Geste::Lancer)],
            /* … chaque variante, sans `_ =>` */
        }
    }
    fn bot_ecran(&self, _: &Lecteur, h: &mut Hasard, _: Style) -> Vec<Coup<Self>> { /* B, puis Espace ; Espace à la fin */ }
    fn pire_cas(c: CasEcran) -> Self { match c { CasEcran::Salon8 => /* … */, /* chaque variante */ } }
}

jeu::exporter!(Punchline);
```

Ce que la forme apporte ici :
- le brouillon est un tenu : seul le dernier compte, son débit est borné, et il compte s'il a été reçu avant l'échéance (PUN-01 à PUN-03) ;
- les réponses, brouillons et votes sont des `Secret` ; la vue de la télé lit `affichees`, copiées au moment du vote ;
- « Je sèche » refusé ne tire rien : la mémoire, générateur compris, est remise (PUN-27) ;
- le signal sonore a l'identité de la phase (PUN-66) ;
- l'horloge par personne et la grâce de 20 s vivent dans l'état, avec `parti_a` ; `Reveil::A` réveille le jeu à la fin de chaque grâce.

### 7.2 Sidequest

```rust
#[derive(Clone, Projeter)]
pub struct Sidequest {
    ecran: Appareil,
    persos: Vec<Perso>,                       // ordre d'arrivée du jeu : il décide des collisions
    fiches: BTreeMap<Appareil, Fiche>,        // points, prénom, look, coupé, parti_a
    prochaine_arrivee: u64,                   // l'ordre d'arrivée est au jeu, pas à l'identifiant
    partie: Partie, paquet: Paquet, curseur: Curseur,
    manche: Option<Manche>, numero_manche: u32,
    pause: Option<Pas>, chat: Oui,
    debut: Ordre,                             // pour refuser une commande périmée
    lignes: BTreeMap<DemandeId, Vec<u8>>,     // résultats à écrire, jusqu'à leur réponse
    prochaine_demande: u64,
    avis_ecriture: Option<Avis>,
}

#[derive(Serialize, Deserialize, Forme)]
pub struct Direction { pub x: Axe, pub y: Axe }
#[derive(Serialize, Deserialize, Forme)]
pub enum Geste { Rejoindre { prenom: Texte<18, PointsDeCode>, look: Look }, Saut, Message(Texte<80, PointsDeCode>), Commande(Commande) }

impl Jeu for Sidequest {
    type Geste = Geste;
    type Tenu = Direction;
    type Vue = Statut;                        // prénom, look, phase, pause, chat : rien du monde
    /* … */
    fn declaration() -> Declaration {
        Declaration {
            nom: "sidequest",
            joueurs: Joueurs::Entre { min: 0, max: 71 },
            appareils: Appareils { telephones: Admis::Oui, manettes: Admis::Non(raison!("pas utilisées aujourd'hui")), max: 72 },
            temps: Temps { hz: Hz(60), rattrapage: 3, fenetre: Duree::ms(0), age_max: Duree::s(1) },   // SQ-02
            pause: Pause::Jeu,                // la pause repousse l'échéance : c'est une règle du jeu
            tenu: PolitiqueTenu::Oui { bail: Bail::Duree(Duree::ms(450)), debit: 60 },                // SQ-01
            fin: Fin::Oui,
            prediction: Prediction::Aucune(raison!("les téléphones ne montrent pas le monde ; la télé est sur l'autorité")),
            heritage: PolitiqueHeritage::Oui,  // les points d'une partie, si la version change en soirée
            /* … */
        }
    }

    fn etape(&mut self, ctx: &Contexte, e: Entree<'_, Self>) -> Result<(), Refus<Avis>> {
        match e {
            Entree::Pas(p) => { self.echeance(ctx, p); self.physique(ctx); self.oublis(ctx, p); Ok(()) }
            Entree::Tenu(t) => self.diriger(ctx, t),       // Tient(Page(d)) ou Tient(Stick(s)) : la même direction
            Entree::Presence(Presence::Part { appareil, .. }) => self.fantome(ctx, appareil),
            Entree::Reponse(id, Reponse::Fait(r)) => self.ligne_ecrite(id, r),   // un échec s'annonce, ne bloque rien
            Entree::Geste(g) => match (g.role, g.quoi) {
                (Role::Ecran, Action::Jeu(Geste::Commande(c))) if c.avance() && g.vu < self.debut => Err(Refus::Muet),
                (Role::Ecran, Action::Jeu(Geste::Commande(c))) => self.commander(ctx, c),
                (Role::Ecran, Action::Touche(t)) => self.raccourci(ctx, t),      // ←, →, P, F, L, Maj+E
                (Role::Telephone, Action::Jeu(x)) => self.joueur(ctx, g.de, x),
                /* … */
            },
            /* … */
        }
    }

    fn horloge(&self, _: &Lecteur) -> Horloge {
        Horloge {
            reveil: Reveil::ChaquePas,
            attend: match &self.manche {
                Some(m) => Attend::Appareils { qui: vec![], echeance: self.pause.map_or(Some(m.fin), |_| None) },
                None => Attend::Appareils { qui: vec![self.ecran], echeance: None },   // l'organisateur fait avancer
            },
            phase: Phase { nom: self.partie.nom(), numero: self.numero_manche.into(), fin: self.partie.est_finie() },
        }
    }
    fn salle(&self) -> Salle {
        Salle {
            accueil: if self.fiches.len() >= 71 { Accueil::Pleine } else { Accueil::Ouverte },
            garde: self.fiches.keys().copied().collect(),   // une fiche oubliée après 90 s sans personnage
        }
    }
    fn demandes(&self) -> Vec<Demande<'_>> {
        self.lignes.iter().map(|(id, l)| Demande { id: *id, quoi: QuoiDemande::Ecrire { flux: "resultats", ligne: l } }).collect()
    }
    fn telephone(&self, l: &Lecteur) -> Montre<Statut> {
        Montre {
            vue: self.statut(l),
            effets: self.vibrations(l),        // id = (manche, appareil) : un point gagné vibre une fois
            reglages: Reglages { orientation: Some(Orientation::Portrait), pad: Some(Pad::stick_et_saut()), /* … */ },
        }
    }
    /* ecran : salle d'arcade en images RGBA, à l'échelle entière dans la zone sûre ; nœuds à clé pour glisser */
}

impl Epreuve for Sidequest {
    type Resume = Points;
    fn invariants(&self, v: &mut Verif) {
        v.exige("personne ne sort du sol", self.persos.iter().all(Perso::sur_le_sol).into());
        v.exige("vitesse au plus 218,5 px/s", self.persos.iter().all(|p| p.vitesse() <= 218.5).into());
        v.exige("comptes + hors zone = participants", self.manche.as_ref().map_or(Oui::Oui, Manche::somme_juste));
    }
    fn transition(a: &Points, b: &Points, v: &mut Verif) {
        v.exige("une relance ne retire jamais un point", (b.partie != a.partie || b.par_appareil_au_moins(a)).into());   // SQ-29
    }
    fn bot_telephone(m: &Montre<Statut>, h: &mut Hasard, s: Style) -> Vec<Coup<Self>> {
        /* rejoindre, marcher vers une zone, sauter, écrire, se couper (Couper), revenir */
    }
    fn bot_ecran(&self, _: &Lecteur, h: &mut Hasard, _: Style) -> Vec<Coup<Self>> {
        /* SUIVANT jusqu'au bout du paquet, TERMINER, puis REJOUER */
    }
    /* … */
}
```

Ce que la forme apporte ici :
- le bail de 450 ms et l'âge d'1 s sont déclarés et tenus par le commun ; un stop passe toujours (SQ-01, SQ-02) ;
- le stick natif de l'app arrive comme `Tenue::Stick`, la croix comme `Tenue::Page` : même appareil, même entrée ;
- l'ordre des collisions est celui que le jeu tient (`prochaine_arrivee`), jamais l'ordre des identifiants ;
- la ligne de résultats est une demande : son échec revient comme une entrée, et la télé l'annonce sans bloquer ;
- un `Suivant` refusé faute de numéro de manche ne fait pas avancer le paquet : la mémoire est remise.

### 7.3 Tier list

```rust
#[derive(Clone, Projeter)]
pub struct TierList {
    plateau: Plateau,
    historique: Historique<Plateau>,          // 200 étapes ; les contenus sont des Rc partagés
    edition: Option<Edition>,                 // le texte en cours forme une seule étape
    fenetre: Fenetre,
    glisser: Option<Glisser>,                 // tuile tenue et place visée, dans l'ordre de l'état
    imports: BTreeMap<DemandeId, Import>,     // plusieurs, dans n'importe quel ordre
    export: Option<(DemandeId, Export)>,      // un seul ; le suivant remplace
    restauration: Option<DemandeId>,
    telechargements: BTreeMap<DemandeId, (&'static str, Rc<[u8]>)>,
    avis: Option<Avis>,
    prochaine: u64,
}
#[derive(Clone, Projeter)]
struct Plateau {
    titre: Texte<500, Utf16>,
    rangs: Vec<Rang>,
    reserve: Vec<TuileId>,
    contenus: BTreeMap<TuileId, Rc<Contenu>>,
    prochaine_tuile: u32, prochain_rang: u32,
}
enum Contenu { Image { octets: Rc<[u8]>, cle: Cle }, Texte(Texte<60, Utf16>) }

#[derive(Serialize, Deserialize, Forme)]
pub enum Heritage { V1(PlateauV1) }        // l'enum versionné ; une V2 se convertit depuis V1

impl Jeu for TierList {
    type Geste = Bouton;                      // Undo, Redo, Reset, Export, Reglages(r), Couleur(c), …
    type Tenu = Aucun;
    type Vue = Aucun;
    type Avis = Avis;                         // « This file is not a tier list backup. Your tier list has not changed. »
    type Heritage = Heritage;

    fn declaration() -> Declaration {
        Declaration {
            nom: "tier-list",
            joueurs: Joueurs::SeulEcran(raison!("une seule main manipule le plateau, par décision de l'hôte")),
            appareils: Appareils {
                telephones: Admis::Non(raison!("jeu solo devant la télé")),
                manettes: Admis::Non(raison!("souris et clavier seulement")),
                max: 1,
            },
            temps: Temps { hz: Hz(60), rattrapage: 0, fenetre: Duree::ms(0), age_max: Duree::s(1) },
            pause: Pause::Aucune(raison!("rien ne bouge sans geste")),
            tenu: PolitiqueTenu::Aucun(raison!("aucun état continu : chaque geste est un événement")),
            fin: Fin::Non(raison!("un éditeur n'a pas de fin")),
            prediction: Prediction::Aucune(raison!("l'écran est la partie qui fait foi")),
            heritage: PolitiqueHeritage::Oui,
            /* … */
        }
    }

    fn etape(&mut self, ctx: &Contexte, e: Entree<'_, Self>) -> Result<(), Refus<Avis>> {
        match e {
            Entree::Geste(g) => match g.quoi {
                Action::Deplace(Deplace::Survol { objet, zone, indice }) => self.viser(objet, zone, indice),   // la vraie tuile bouge en direct
                Action::Deplace(Deplace::Depose { objet, zone, indice }) => self.action(ctx, Act::Deplacer(objet, zone, indice)),
                Action::Deplace(Deplace::Annule { .. }) => self.lacher(),
                Action::Saisie(s) => self.saisir(ctx, s),                 // une étape par texte, close par la fin de saisie
                Action::Fichiers(f) => self.recevoir(ctx, f),             // images : une demande par fichier ; sauvegarde : taille d'abord
                Action::Touche(t) => self.raccourci(ctx, t),              // Ctrl/Cmd+Z, Maj+Z, Y, Échap ; le commun les retient dans un champ
                Action::Jeu(b) => self.bouton(ctx, b),
                Action::Pointeur(_) => Err(Refus::Muet),
            },
            Entree::Reponse(id, r) => self.reponse(ctx, id, r),           // n'arrive que si `id` est encore listé
            Entree::Pas(_) | Entree::Presence(_) => Ok(()),
            Entree::Tenu(t) => match t.valeur { /* Aucun : impossible à construire */ },
        }
    }

    fn horloge(&self, _: &Lecteur) -> Horloge {
        Horloge { reveil: Reveil::Jamais, attend: Attend::Rien(raison!("un éditeur attend un geste, sans échéance")), phase: Phase { nom: "edition", numero: 0, fin: Oui::Non } }
    }
    fn demandes(&self) -> Vec<Demande<'_>> {
        // Reset et restauration vident `imports`, `export` et `restauration` : tout est annulé (TL-44 à TL-46).
        let mut d: Vec<_> = self.imports.iter().map(|(id, i)| Demande { id: *id, quoi: QuoiDemande::Image { fichier: i.fichier, carre: 160, format: FormatImage::WebP(85) } }).collect();
        if let Some((id, Export::Dessin(scene))) = &self.export { d.push(Demande { id: *id, quoi: QuoiDemande::Rendu { scene, echelle: 2, format: FormatImage::Png } }); }
        if let Some(id) = self.restauration { d.push(Demande { id, quoi: QuoiDemande::Lire { fichier: self.fichier_restaure(), max: 64 << 20 } }); }
        d.extend(self.telechargements.iter().map(|(id, (nom, o))| Demande { id: *id, quoi: QuoiDemande::Telecharger { nom, octets: o } }));
        d
    }
    fn heritage(&self) -> Option<Heritage> { Some(Heritage::V1(self.plateau.v1())) }
    fn ressource(&self, cle: Cle) -> Option<&[u8]> { self.plateau.image(cle) }
    fn salle(&self) -> Salle { Salle { accueil: Accueil::Fermee, garde: vec![] } }
    /* ecran : la page en scène défilante ; tuiles en nœuds déplaçables, rangs et réserve en zones, poubelle en zone ;
       champs en place à clé (rang, contenu) ; fenêtres sur voile ; avis du jeu à côté des avertissements du commun */
}

impl Epreuve for TierList {
    type Resume = Empreinte;                  // l'empreinte du plateau et de l'historique
    fn invariants(&self, v: &mut Verif) {
        v.exige("chaque tuile placée une fois", self.plateau.placees_une_fois());
        v.exige("chaque tuile placée a un contenu", self.plateau.contenus_complets());
        v.exige("historique au plus 200", (self.historique.len() <= 200).into());
    }
    fn bot_ecran(&self, _: &Lecteur, h: &mut Hasard, s: Style) -> Vec<Coup<Self>> {
        /* suites au hasard, valides ou non : dépôts, saisies hostiles, fichiers abîmés, Undo, Reset */
    }
    fn bot_telephone(m: &Montre<Aucun>, _: &mut Hasard, _: Style) -> Vec<Coup<Self>> { match m.vue {} }
    /* … */
}
```

Ce que la forme apporte ici :
- l'état de dizaines de Mo et ses 200 étapes se copient, se comparent et s'empreintent par blocs écrits ; les `Rc` partagés restent partagés, puisque rien n'est sérialisé (TL-20, TL-22) ;
- le plateau survit au rechargement sans une ligne de sauvegarde : c'est la mémoire. L'héritage ne sert qu'aux changements de version et au fichier de secours ;
- une réponse d'import, d'export ou de lecture qui arrive après un Reset ne touche à rien : sa demande n'est plus listée ;
- aucun pas pendant le glisser : le défilement au bord est fait par le composant, en vitesse par seconde.

---

## 8. Ce que l'interface rend impossible

| Famille de bugs | Par quelle forme | Leçons |
|---|---|---|
| Un refus qui change l'état ou fait avancer le hasard | le moteur remet la mémoire, générateur compris | PUN-27, OUT-171, HIS-44, CAT-94, TL-02, SQ-25 |
| Une lecture (vue, bot, invariant) qui modifie l'état | toute lecture est un appel remis | TL-01 |
| Une entrée fautive rejouée en boucle par la reprise | l'entrée fautive est isolée et sort du journal | contrat, « La forme d'un jeu » |
| Un pas qui ne rend pas la main | carburant compté dans le module, déterministe | contrat, « Chaque pas a un budget » |
| NaN, infini, sous-normal, valeur hors bornes à l'entrée | `Fini`, `Axe`, `Texte` se valident au décodage | HIS-45, OUT-175, CAT-04 |
| Octets restants, longueur absurde | décodage strict | OUT-146, REM-58, REM-59 |
| Le jeu lit l'heure, le hasard du système, un fichier | aucun import, `no_std` | OUT-145, PUN-89, SAL-93 |
| Fonctions flottantes qui diffèrent entre natif et module | absentes de `core` ; NaN canoniques | HIS-50, OUT-144, SQ-68 |
| `HashMap` dont l'ordre diffère | absent de `no_std` | règles d'écriture |
| Un secret dans la vue d'un téléphone ou de la télé | `Secret` n'est pas sérialisable ; la vue passe par un lecteur qui ne le lit pas | CAT-46 |
| Un rejeu qui compare une partie de l'état seulement | l'empreinte couvre toute la mémoire | TL-21, SAL-158 |
| Une réponse à une demande annulée ou remplacée | une réponse n'entre que si sa demande est listée | TL-44, TL-45, TL-46 |
| Un effet rejoué par un réaffichage, une reconnexion, un rejeu | l'effet a une identité et un instant, dans la vue | PUN-66 |
| Un minuteur du jeu qui survit à sa phase ou à sa partie | il n'y a pas de minuteur : `Reveil` se lit dans l'état | PUN-34 |
| Un geste d'une phase passée appliqué à la suivante | `vu` dans chaque geste | PUN-38, CAT-44 |
| L'identité d'un appareil qui départage une simultanéité | identifiants tirés au hasard ; départage par la graine ; permutation éprouvée | CAT-27 |
| Un lien, une session, un onglet visibles du jeu | le jeu ne voit que des appareils | SAL-18, SAL-19, SAL-21, SAL-36, SAL-38, SAL-39 |
| Une direction tenue qui marche seule ou vient de trop loin | bail et âge déclarés, tenus par le commun ; un relâchement passe toujours | SQ-01, SQ-02, CAT-25 |
| Des états périmés livrés d'un bloc | numéro croissant, un tenu par pas | REM-13, REM-14 |
| Une commande d'organisateur venue d'un téléphone | `Role::Ecran` n'existe que sur l'ordinateur de la soirée | cahier de Sidequest, § 2 |
| Une capacité dont le jeu dépend | il n'existe pas de champ pour en exiger une | OUT-211, REM-54 |
| Une exemption par oubli | chaque champ de la déclaration est obligatoire, une exemption porte sa raison | HIS-04, OUT-03, OUT-04, HIS-43 |
| Une page d'une autre version qui décode de travers | `formes` et empreinte du module | SAL-41, PUN-44, REM-77 |
| Une sauvegarde d'une autre version lue de travers | seul l'héritage passe, enum versionné, `Illisible` explicite | HIS-51, TL-13, TL-67 |
| Un bot qui joue ce que l'écran n'offre pas | `bot_telephone` ne reçoit que la vue | CAT-78 |
| Une progression devinée dans le DOM | `Phase` se lit dans l'état | HIS-13, OUT-29 |

Ce que la forme ne règle pas, et qui reste aux pressions :
- l'équité d'une résolution simultanée dans un même pas : c'est au jeu de rassembler et d'évaluer sur l'état d'avant (CAT-28) ;
- une révélation trop tôt : c'est une décision du jeu, que ses invariants tiennent ;
- une règle fausse, un score faux : les invariants, les mutations et les cahiers.

---

## 9. Risques et questions ouvertes

### Risques

1. **Le coût des pages sales.** Chaque écriture en mémoire gagne un test et parfois une copie. Le ralentissement est inconnu : il faut le mesurer sur les trois jeux avant de s'y engager. Repli : copier toute la mémoire pour un petit état, réécrire seulement pour un grand.
2. **Une seconde mémoire dans le module réécrit.** La réécriture ajoute une mémoire. Le support de plusieurs mémoires par chaque navigateur visé, Safari compris, est à vérifier. Repli : une zone réservée en haut de la mémoire du jeu, interdite à ses écritures.
3. **La mémoire ne rétrécit jamais.** Une mémoire WebAssembly grandit et ne rend rien. Une soirée fragmentée peut atteindre le maximum. La mesure doit porter sur les octets vivants (`vivant`), et le maximum déclaré doit tenir les pires soirées.
4. **Une mémoire libérée garde ses octets.** Un secret peut survivre dans un bloc libre. L'allocateur remet à zéro, mais seul le chiffrement des instantanés fait la garantie.
5. **Une seule porte.** Un bug de la colle de `jeu` touche tous les jeux. Elle doit rester petite, fuzzée, et peut-être prouvée.
6. **Les lectures à chaque image.** 71 téléphones à 60 images par seconde, c'est plus de 4 000 vues calculées par seconde dans Sidequest. Chacune est petite, mais le total est à mesurer.
7. **La déclarativité demande de la discipline.** Un jeu qui oublie de garder un appareil arrivé le retire aussitôt ; un jeu qui oublie de retirer une demande finie la laisse listée. L'épreuve voit les deux (aucun appareil sans vue, demandes bornées), mais la forme ne les empêche pas.
8. **`no_std` ferme des crates.** Une crate tierce qui exige `std` est exclue. C'est voulu pour `HashMap` et l'horloge, mais une crate de physique ou d'images peut manquer.
9. **Un tenu fusionné perd un appui bref** quand le Wi-Fi livre par rafales. Un jeu qui compte des appuis doit en faire des événements (REM-16, REM-17).
10. **L'ordre dans un pas et le replacement.** Avec une fenêtre non nulle, un geste replacé change le rang des entrées de son pas. Une comparaison `vu < debut` sur des entrées du même pas peut basculer. Aucun des trois jeux n'a de fenêtre ; il faudra le trancher avant le premier qui en a une.
11. **Les travaux lourds.** Encoder 30 Mo de sauvegarde en un appel peut dépasser le budget. Le jeu doit alors découper son travail sur plusieurs pas, ou le commun proposer des demandes qui le font à sa place.
12. **Le sélecteur de fichiers exige un geste du joueur.** Le navigateur n'ouvre un sélecteur que dans le clic. Le composant de la scène doit l'ouvrir lui-même, sans aller-retour avec le Worker.

### Questions ouvertes

1. Les budgets en instructions par sorte d'appel, et la profondeur d'appel maximale : à mesurer, puis à écrire avec leur date.
2. La déclaration dans une section personnalisée du module, lisible sans rien exécuter, plutôt que par Bonjour ?
3. La colle d'export : une macro avec le seul attribut `unsafe` du jeu, ou une crate générée par `soiree`, hors du code du jeu ?
4. Les avis d'une action acceptée (« Backup restored… ») vivent dans l'état, ceux d'un refus passent par le commun. Faut-il un seul emplacement, que la vue remplit et qu'un refus remplace jusqu'à la vue suivante ?
5. Le `bot_ecran` lit l'état public. Faudrait-il, comme pour le téléphone, qu'il ne lise que la scène ?
6. La tolérance d'un effet en retard (250 ms dans Punchline) : par effet, par jeu, ou commune ?
7. La touche du diagnostic et celle de la marque : réservées par le commun pour tous les jeux (F8, et laquelle pour le diagnostic ?), alors que Sidequest utilisait D.
8. La prédiction sérialise la projection, et perd le partage des `Rc`. Faut-il une projection par blocs de mémoire, plutôt que par sérialisation, le jour où un jeu prédit ?
9. Le composant déplaçable calcule la place visée pour une zone en lignes. Suffit-il, ou faut-il donner au jeu la géométrie dessinée pour qu'il calcule lui-même ?
10. Le pad natif de l'app : le stick arrive en `Tenue::Stick`, les boutons en gestes du jeu préparés par la vue. Le contrat de l'app (`remote/CONTRAT.md`) doit le porter.
11. `Joueurs::Entre { min: 0, … }` pour Sidequest : l'épreuve doit-elle exiger un minimum non nul pour savoir combien de bots lancer ?
12. Quand le relais garde l'instantané chiffré d'un jeu à un seul appareil, comme la tier list, ou faut-il seulement le stockage local de l'ordinateur ?
