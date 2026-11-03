# Proposition · les types entre le commun et un jeu

> **Statut.** Proposition, à relire par le mainteneur. Elle ne fait pas foi : [`MANIFESTE.md`](../MANIFESTE.md) et [`CONTRAT.md`](../CONTRAT.md) font foi, et ce document se corrige s'il les contredit.
>
> **Angle.** La sûreté de type de bout en bout. Chaque fois qu'une erreur connue peut devenir une erreur de compilation, ou une valeur qu'aucun type ne peut représenter, on le fait. Quand on n'y arrive pas, on le dit, et on nomme la pression qui prend le relais.
>
> **Date.** 3 novembre 2026, branche `refonte`. Aucun code n'existe encore : les signatures sont une cible, pas un relevé. Aucun chiffre de ce document n'est mesuré.

## 0. En une page

Trois pièces, et une seule définition pour ce qu'elles échangent :

- **la crate `jeu`**, que chaque jeu utilise. Elle est `#![no_std]` avec `alloc`. Elle définit les types que le jeu écrit (état, gestes, vues, effets, demandes, secrets) et les traits qu'il implémente ;
- **la crate `soiree-abi`**, la définition unique des messages entre le moteur et le module. Le moteur et `jeu` en dépendent tous les deux ;
- **le moteur**, dans le commun. Il charge le module, le réécrit, l'appelle, garde son journal, le reprend après une panne, et compare deux exécutions.

```
        un appareil (page du commun, ou app)              l'ordinateur de la soirée
  ┌─────────────────────────────────────┐        ┌──────────────────────────────────────────┐
  │ rendu du commun : Page<A> en DOM     │        │ moteur du commun                         │
  │   champs natifs, boutons, croix…     │ relais │   horloge, graine, journal, reprise      │
  │ renvoie (vue n°, élément, donnée)    │◀──────▶│   validation de chaque entrée            │
  │                                      │ octets │   ┌────────────────────────────────────┐ │
  │ prédiction (si le jeu en déclare) :  │chiffrés│   │ module du jeu : wasm, aucun import  │ │
  │   le même module, sur sa projection  │        │   │   crate jeu  +  règles du jeu       │ │
  └─────────────────────────────────────┘        │   └────────────────────────────────────┘ │
                                                 │ télé : Ecran<A> dessiné par le commun    │
                                                 └──────────────────────────────────────────┘
```

Huit idées portent tout le reste :

1. **Le module ne reçoit que des valeurs déjà validées.** Un geste est d'un type `Entrant`, qui n'a pas de flottant nu, pas de texte nu, pas de longueur sans borne. NaN, l'infini et un prénom invisible ne sont pas représentables à la frontière.
2. **Un appareil ne nomme jamais une action.** Il désigne un élément de la vue qu'il a reçue, et joint une donnée du type que cet élément annonce. L'action complète est celle que le jeu y avait mise, retrouvée par l'autorité. Un téléphone ne peut donc envoyer que ce que son écran offrait, à ce moment-là.
3. **Un refus est un retour en arrière de la mémoire, fait par le moteur.** Le jeu peut écrire ce qu'il veut avant de refuser : le moteur rend la mémoire à l'identique, hasard compris.
4. **Un secret se lit à travers un lecteur que seul le commun pose.** Dans une vue de télé, aucun secret ne se lit. Sur un appareil, un secret absent arrête la prédiction de ce geste.
5. **Effets et demandes se déduisent de l'état, comme la vue.** Ils ont une identité. Un réaffichage les recalcule à l'identique, et le commun ne joue ou ne lance que ce qu'il n'a pas déjà joué ou lancé. Une demande qui disparaît de l'état est annulée, et sa réponse ne revient jamais.
6. **Le temps qu'on stocke n'est pas le temps qu'on lit.** `Pas` est un instant passé, qu'on garde dans l'état. `Maintenant` n'existe que pendant une entrée et ne se garde pas. Une échéance ne se crée que depuis `Maintenant`.
7. **Toute déclaration est un champ obligatoire.** « Sans objet » est une valeur, avec sa raison. Omettre un champ ne compile pas.
8. **Le moteur réécrit le module, et le module se laisse réécrire.** Compteur d'instructions, NaN canoniques, mémoire bornée, globales exportées : ce qui rend l'arrêt, la copie et la comparaison possibles est ajouté au chargement, pas promis par le jeu.

---

## 1. L'API de la crate `jeu`

### 1.1 La forme de la crate

```rust
// jeu/src/lib.rs
#![no_std]
extern crate alloc;

pub mod donnee;      // Donnee, Entrant, Fini, Axes, Direction, Borne, Texte, Bloc, Appareils
pub mod temps;       // Pas, Duree, Maintenant, Echeance
pub mod appareil;    // Appareil, Session, Presence, Capacites
pub mod geste;       // Role, Evenements, Continus, Gabarit, Geste, Continu
pub mod secret;      // Secret, SecretDe, Lecteurs, SecretAbsent
pub mod hasard;      // Hasard
pub mod vue;         // Ecran, Scene, Page, éléments, Effet
pub mod demande;     // Demande, Reponse
pub mod declaration; // Declaration, Usage, Raison, Salle…
pub mod epreuve;     // Bots, Invariants, Verif, Choix
pub mod version;     // Etat, Ancien, Neuf, Incompatible
pub mod prelude;

pub use jeu_derive::{Continus, Donnee, Enumerable, Evenements};
```

Le jeu est une crate de bibliothèque `#![no_std]` et `#![forbid(unsafe_code)]`. Le module WebAssembly est une seconde crate, générée par l'outil `soiree` et jamais écrite à la main (section 1.14). C'est elle seule qui porte les exports, et le peu de code `unsafe` qu'ils demandent.

`no_std` n'est pas un détail. Il fait respecter par la forme trois règles d'écriture que le contrat ne tenait jusqu'ici que par des lints :

| Règle du contrat | Ce que `no_std` en fait |
|---|---|
| Ni `HashMap` ni `HashSet` | Ils n'existent pas dans `alloc`. |
| Ni `Instant::now` ni `SystemTime::now` | `std::time` n'existe pas. `Instant::now` ne peut plus paniquer dans le module (OUT-145). |
| Ni `sin`, `cos`, `powf`… des flottants | Ces méthodes vivent dans `std`, pas dans `core`. Il reste `libm`, réexporté par `jeu::math` (HIS-50, OUT-144). |

La copie native du jeu, pour aller vite pendant le développement, compile la même crate : elle n'a pas davantage accès à `std`.

Le jeu n'a pas le droit d'avoir un `static` à mutabilité intérieure (`static X: Cell<…>`, `OnceCell`, `thread_local!`). `soiree check` le refuse. Sans cela, une vue pourrait lire l'état complet par une porte dérobée (section 4).

### 1.2 Les données

```rust
/// Une valeur qui peut vivre dans l'état, dans une vue ou dans un message.
/// Seule la dérivation `#[derive(Donnee)]` l'implémente pour un type du jeu.
pub trait Donnee: Sized + serde::Serialize + serde::de::DeserializeOwned + 'static {
    /// Forme de la donnée, calculée à la compilation. Son empreinte entre dans
    /// la version du jeu et dans la conversion des sauvegardes (1.13).
    const SCHEMA: Schema;
    /// Parcourt les flottants, les secrets et les blocs de la valeur.
    fn visiter(&self, v: &mut dyn Visiteur);
}

/// Une valeur qui peut venir de l'extérieur du module : d'un appareil, d'un fichier, d'une réponse.
/// Plus stricte que `Donnee` : ni flottant nu, ni `String`, ni collection sans borne.
pub trait Entrant: Donnee {}
```

`#[derive(Donnee)]` génère `Serialize` et `Deserialize` par `serde`, et refuse à la compilation :
- un champ dont le type n'implémente pas `Donnee` (donc toute `HashMap`, tout `Instant`, toute fermeture) ;
- les attributs `serde` qui élargissent ce qu'on accepte : `untagged`, `tag`, `flatten`, `default`, `skip` (REM-60, PUN-91, SAL-119). Le format postcard n'a pas d'étiquette : un variant est un numéro, et son ordre fait partie du schéma.

`Donnee` est implémentée par `jeu` pour les entiers, `bool`, `char`, `String`, `Vec`, `Box`, `Option`, les tableaux, les tuples, `BTreeMap`, `BTreeSet`, et pour `f32` et `f64`. `Entrant` ne l'est pas pour `f32`, `f64`, `String` ni `Vec<T>` sans borne.

**Les flottants.** Un flottant peut vivre dans l'état, parce qu'une physique en a besoin. Il ne peut pas entrer :

```rust
/// Un flottant fini, ni NaN, ni infini, ni sous-normal. `Entrant`.
pub struct Fini<F>(F);
impl Fini<f32> {
    pub const fn new(x: f32) -> Option<Fini<f32>>;
    pub const fn get(self) -> f32;
}

/// Des axes de stick : finis, chacun dans [-1, 1], de norme au plus 1, zone morte
/// déjà appliquée par le commun avec le réglage de l'appareil. `Entrant`.
pub struct Axes { x: Fini<f32>, y: Fini<f32> }
impl Axes {
    pub const ZERO: Axes;
    pub fn x(self) -> f32;
    pub fn y(self) -> f32;
}

/// Une croix : chaque axe vaut -1, 0 ou 1, opposés déjà annulés. `Entrant`.
pub struct Direction { pub x: Signe, pub y: Signe }
pub enum Signe { Moins, Zero, Plus }

/// Un entier dans [MIN, MAX], vérifié au décodage. `Entrant`.
pub struct Borne<const MIN: i64, const MAX: i64>(i64);
```

Un flottant de l'état est vérifié fini à chaque pas, pendant l'épreuve, par `Donnee::visiter` : c'est un invariant que le commun ajoute à tous les jeux, sans que le jeu l'écrive. Dans le module, le moteur rend les NaN canoniques (2.5) : même un NaN se rejoue au bit près (SQ-68).

**Les textes.** Un texte venu d'un appareil n'est jamais une `String`. Il porte sa limite dans son type :

```rust
/// Une limite de texte. Une seule définition, partagée par les règles et par le champ
/// de la page qui le saisit (TL-27, SQ-40).
pub trait Limite: 'static {
    const MAX: u32;
    const UNITE: Unite;                 // Capitales | Utf16 | PointsDeCode | Graphemes
    const POLICE: Option<PoliceId>;     // ce qui ne se dessine pas dans cette police est retiré
}

jeu::limite! {
    /// 12 caractères comptés en capitales, dessinables avec Rubik.
    pub Prenom = 12 capitales, police RUBIK;
}

/// Un texte nettoyé par le commun : NFC, contrôles et invisibles retirés (Cf compris),
/// liants et sélecteurs gardés seulement dans un emoji, espaces réduites, coupé entre
/// deux graphèmes, au plus `L::MAX` unités, non vide. `Entrant`.
pub struct Texte<L: Limite> { /* texte, clé, découpe en graphèmes, poids */ }

impl<L: Limite> Texte<L> {
    pub fn as_str(&self) -> &str;
    /// Clé de comparaison : casse ignorée, sans sélecteurs ni liants, confusables ramenés.
    /// La seule fonction de clé de tout le jeu (CAT-117).
    pub fn cle(&self) -> &CleTexte;
    pub fn poids(&self) -> u32;
    /// Ajoute un suffixe et raccourcit le texte entre graphèmes pour qu'il tienne (PUN-25).
    pub fn avec_suffixe(&self, suffixe: &Texte<Libre>) -> Texte<L>;
    pub fn borne<M: Limite>(&self) -> Option<Texte<M>>;
}

/// Un texte écrit dans les sources, validé à la compilation.
macro_rules! texte { /* texte!(Prenom, "Patouille") */ }
```

Le nettoyage, la coupe et la clé sont calculés une fois, par le commun, avant que le texte n'entre dans le journal. Le texte arrive dans le module avec sa clé et sa découpe : le module n'embarque aucune table Unicode, et un journal rejoué avec un commun plus récent garde le texte qu'il avait (CAT-83 à CAT-85, CAT-88, PUN-20 à PUN-24, TL-28). La police déclarée donne la table de ce qui se dessine, tirée du fichier de police à la compilation (SQ-38, SQ-39). Les noms réservés d'un jeu se déclarent avec `texte!` et se comparent par `cle()`.

**Les blocs.** Une image ou un fichier ne vit pas dans la mémoire du module :

```rust
/// Des octets immuables, gardés par le commun, nommés par leur empreinte (BLAKE3).
/// Le jeu ne lit jamais ces octets : il les nomme, les montre, les transforme ou les donne
/// à télécharger par une demande. `Entrant`.
pub struct Bloc<T: SorteBloc> { empreinte: [u8; 32], taille: u64, _t: PhantomData<T> }
pub enum Image {}   // WebP ou PNG, décodable, dimensions connues
pub enum Png {}
pub enum Brut {}    // un fichier reçu, pas encore lu
impl<T: SorteBloc> Bloc<T> {
    pub fn taille(&self) -> u64;
    pub fn empreinte(&self) -> &[u8; 32];
}
impl Bloc<Image> { pub fn dimensions(&self) -> (u16, u16); }
```

L'état d'une partie est la mémoire du module **et** l'ensemble des blocs qu'elle nomme. Un bloc ne change jamais : le copier, c'est copier son nom. Deux étapes d'historique qui gardent la même image partagent le même bloc (TL-20). Comparer deux états, c'est comparer deux mémoires de quelques Mo, pas de quelques dizaines de Mo (TL-22).

**Les collections d'appareils.**

```rust
/// Un appareil, tel que le commun l'a créé. Ni `Ord`, ni `Hash`, ni constructeur public.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Appareil(/* privé */);

/// Une table d'appareils, dans l'ordre où ils y sont entrés.
pub struct Appareils<V> { /* privé */ }
impl<V> Appareils<V> {
    pub const fn new() -> Self;
    pub fn get(&self, a: Appareil) -> Option<&V>;
    pub fn get_mut(&mut self, a: Appareil) -> Option<&mut V>;
    pub fn inserer(&mut self, a: Appareil, v: V) -> Option<V>;
    pub fn retirer(&mut self, a: Appareil) -> Option<V>;
    pub fn iter(&self) -> impl Iterator<Item = (Appareil, &V)>;   // ordre d'insertion
    pub fn len(&self) -> usize;
}
```

`Appareil` n'est pas ordonné. Un jeu ne peut donc pas trier par identité d'appareil, ni départager deux joueurs par elle : il ne peut que suivre l'ordre où les appareils sont entrés, qui est l'ordre des entrées, fixé par le commun. Le commun peut alors permuter les identifiants qu'il crée et vérifier que rien ne change (CAT-27, section 3.7).

### 1.3 Le temps

```rust
pub const PAS_PAR_SECONDE: u32 = 60;

/// Un instant passé de la partie, en pas. Se garde dans l'état.
#[derive(Donnee, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pas(u64);

/// Une durée, en pas entiers. Jamais de somme de flottants (CAT-15).
#[derive(Donnee, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Duree(u64);
impl Duree {
    pub const fn pas(n: u64) -> Duree;
    pub const fn ms(n: u64) -> Duree;          // arrondi au pas supérieur
    pub const fn s(n: u64) -> Duree;
    pub const fn en_ms(self) -> u64;
    pub const fn secondes_restantes(self) -> u64;   // arrondi au-dessus (SQ-35)
}

/// L'instant de l'entrée en cours. N'existe que le temps d'une entrée :
/// ni `Donnee`, ni `Clone`, ni `'static`. On ne peut pas le garder dans l'état.
pub struct Maintenant<'e> { pas: Pas, _e: PhantomData<&'e ()> }
impl Maintenant<'_> {
    pub fn pas(&self) -> Pas;
    pub fn depuis(&self, p: Pas) -> Duree;      // saturé à zéro
}

/// Une échéance. Se garde dans l'état. Ne se crée que depuis `Maintenant`.
#[derive(Donnee, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Echeance(Pas);
impl Echeance {
    pub fn dans(m: &Maintenant<'_>, d: Duree) -> Echeance;
    pub fn atteinte(&self, m: &Maintenant<'_>) -> bool;
    pub fn restant(&self, m: &Maintenant<'_>) -> Duree;
    /// Repousse d'une durée mesurée, comme une pause (sidequest 4.8).
    pub fn repoussee(self, d: Duree) -> Echeance;
}
```

Il n'existe pas de `Echeance + Duree`. Une phase qui s'enchaîne sur la précédente part donc de `Maintenant`, pas de l'échéance d'avant : après un gel, la phase suivante repart du réveil (PUN-34). `repoussee` reste possible, pour une pause : la forme oriente, elle ne prouve pas (section 9, R-08).

Le jeu ne programme aucun minuteur. Il dit quand il veut être réveillé, par `Jeu::echeance`, qui se calcule depuis l'état. Le moteur lui livre un `Pas` à cet instant-là.

### 1.4 Appareils et présence

```rust
pub enum Presence {
    Arrivee { appareil: Appareil, sorte: SorteAppareil, session: Session, capacites: Capacites },
    Depart { appareil: Appareil, cause: CauseDepart },
    Retour { appareil: Appareil, session: Session, capacites: Capacites },
    Capacites { appareil: Appareil, capacites: Capacites },
}
pub enum SorteAppareil { Telephone, Manette }
pub enum CauseDepart { Silence, Fermeture, Voulu }

/// Distingue deux onglets ou deux ouvertures d'un même appareil (OUT-183). Opaque.
#[derive(Donnee, Clone, Copy, PartialEq, Eq)]
pub struct Session(/* privé */);
```

Le jeu ne voit jamais un lien, un jeton, un canal, un onglet. Il voit un `Appareil` qui arrive, part avec une cause, et revient (SAL-15, SAL-16). L'app iPhone et sa page sont un seul `Appareil` (SQ-88). Une page fermée pour laisser place au pad n'est pas un `Depart` (SQ-89). Un onglet qui en remplace un autre donne un `Retour` avec une nouvelle `Session`.

Le commun ne libère jamais une place seul. Il ne fait qu'annoncer. Le jeu décide ce que veut dire une absence (pause, tour sauté, place gardée), et c'est lui qui dit, par son contexte, quand oublier ou retirer un appareil (1.7).

```rust
/// Ce qu'un appareil peut faire, annoncé à l'arrivée avec l'état présent (REM-43),
/// puis à chaque changement. Des capacités, jamais une technologie.
pub struct Capacites {
    pub vibration: Capacite<()>,
    pub mouvement: Capacite<RepereCapteurs>,   // capteurs lus dans les axes de l'interface
    pub orientation: Orientation,              // toujours connue, demi-tour compris (REM-44)
    pub son: Capacite<Son>,                    // SansGeste | ApresGeste
    pub ecran_allume: Capacite<()>,
    pub micro: Capacite<()>,                   // jamais en http sur une adresse locale (OUT-211)
    pub pad_natif: Capacite<PadNatif>,
    pub prenom_propose: Option<Texte<Libre>>,
}
pub enum Capacite<T> { Oui(T), Non(Absence) }
pub enum Absence { NonSupportee, ContexteNonSecurise, Refusee, PasEncore }
pub enum Orientation { Portrait, PortraitInverse, PaysageGauche, PaysageDroite }
```

Un jeu reste jouable sans aucune capacité : l'épreuve le joue avec toutes à `Non` (section 6).

### 1.5 Les gestes

Un geste a une origine, et l'origine se lit dans le type.

```rust
/// Les gestes d'un rôle : ses événements, comptés une fois, et ses états continus.
pub trait Role: 'static {
    type Evenement: Evenements;
    type Continu: Continus;
}

/// Un rôle sans geste. `Aucun` n'a pas de valeur : `match x {}` suffit.
pub enum Aucun {}
impl Role for Aucun { type Evenement = Aucun; type Continu = Aucun; }
```

Un jeu a deux rôles : `Telephone`, les gestes d'un téléphone, et `Tele`, les gestes de l'ordinateur de la soirée (sa vue, son clavier, sa souris). Les manettes arrivent en entrées typées du commun (`Manette`), qui ne demandent pas de type au jeu. Le décodeur du canal d'un téléphone ne connaît que les gestes du rôle `Telephone` : une commande d'organisateur n'y existe pas (SQ-21, SQ-22, PUN-48).

**Événements et états continus.** Le jeu écrit deux enums par rôle :

```rust
#[derive(Evenements)]
pub enum GesteTel {
    Personnage(Perso),                          // un bouton : pas de donnée
    Prenom(#[donnee] Texte<Prenom>),            // un champ : la donnée est le texte
    Envoyer { duel: u8, #[donnee] cases: Champs<Reponse> },
    #[jamais_perime]
    FermerAvis,
}

#[derive(Continus)]
pub enum ContinuTel {
    /// Le dernier gagne, gardé même si l'appareil se tait. Au plus 4 par seconde.
    #[dernier(debit = 4)]
    Brouillon { duel: u8, #[donnee] cases: Champs<Reponse> },
    /// Tenu : relâché quand l'appareil se tait 450 ms, et jamais plus vieux qu'1 s.
    #[tenu(silence = 450 ms, age_max = 1 s)]
    Aller(#[donnee] Direction),
}
```

Ce que dit le type, et ce que le commun en fait :

| Sorte | Transport | Livraison au jeu | Leçons |
|---|---|---|---|
| Événement | numéroté par appareil, file qui survit aux coupures et aux redémarrages, accusé | exactement une fois, dans l'ordre, même en retard | SAL-56, SAL-58, SAL-103, REM-07 |
| Continu `dernier` | état complet, numéroté de façon croissante à travers les reconnexions, un seul en attente | le plus récent par clé, au débit déclaré ; jamais relâché | REM-13, REM-14, PUN-01 à PUN-03 |
| Continu `tenu` | idem, rappelé par la page à un tiers du silence | le plus récent par clé ; relâché au silence, au départ, à la perte du premier plan, à une pause du commun ; jamais plus vieux que `age_max` | SQ-01, SQ-02, REM-10, REM-11, REM-18, CAT-25 |

La **clé** d'un continu est son variant et ses champs fixes : `Brouillon { duel: 3 }` et `Brouillon { duel: 4 }` sont deux états distincts, dont chacun garde son dernier. La dérivation l'écrit seule.

Un continu `tenu` a une **valeur de repos**, donnée par le type de sa donnée (`Direction::ZERO`, `Axes::ZERO`, `Appui::Relache`). Le commun livre cette valeur pour relâcher, avec sa cause. Un « stop » passe toujours, même vieux, parce qu'il est le repos (SQ-02).

Un **appui** se déclare comme deux événements, l'enfoncement et le relâchement, comptés chacun une fois. Un appui plus court qu'un échantillon ou qu'un envoi atteint quand même le jeu (REM-16, REM-17, SQ-99). Un jeu qui compte des appuis prend un `BoutonTenu`, qui produit des événements ; un jeu qui veut un état tenu prend une `Croix` ou un `Stick`, qui produisent un continu.

**La péremption.** Par défaut, un événement est périmé si la phase a changé entre la vue où il a été fait et l'état où il arrive. Le module le refuse avant d'appeler le jeu, avec l'avis que le jeu a choisi pour ce cas (`avis_perime`). `#[jamais_perime]` le retire, pour une commande qui fixe une valeur (pause, chat). Un vote du duel précédent ne tombe jamais sur le suivant, et un Espace pressé deux fois ne passe qu'une phase (PUN-38, SQ-23, SAL-115, CAT-44).

**La prédiction.** Par défaut, rien ne se prédit. `#[predit]` sur un variant le permet (section 4).

**Le gabarit.** C'est la pièce qui relie un geste à la vue qui l'a offert.

```rust
/// Une action du rôle `A`, avec tous ses champs sauf la donnée `D`, que l'appareil fournira.
/// `S` dit sa sorte : `Evenement`, `Dernier` ou `Tenu`. Construit seulement par la dérivation.
pub struct Gabarit<A, D: Entrant, S> { prefixe: Vec<u8>, _t: PhantomData<(A, D, S)> }
pub enum Evenement {}
pub enum Dernier {}
pub enum Tenu {}
```

`#[derive(Evenements)]` génère un constructeur par variant :

```rust
impl GesteTel {
    pub fn personnage(p: Perso) -> Gabarit<GesteTel, (), Evenement>;
    pub fn prenom() -> Gabarit<GesteTel, Texte<Prenom>, Evenement>;
    pub fn envoyer(duel: u8) -> Gabarit<GesteTel, Champs<Reponse>, Evenement>;
}
impl ContinuTel {
    pub fn brouillon(duel: u8) -> Gabarit<ContinuTel, Champs<Reponse>, Dernier>;
    pub fn aller() -> Gabarit<ContinuTel, Direction, Tenu>;
}
```

La donnée est toujours le dernier champ du variant ; la dérivation le vérifie. Dans le format postcard, l'encodage d'un variant est son numéro suivi de ses champs dans l'ordre. L'action complète est donc exactement `prefixe ++ encodage(donnée)`. L'autorité la reconstitue sans connaître les types du jeu, puis le module la décode et la valide.

Les éléments de vue ne prennent que le gabarit qui leur convient :

```rust
Bouton::new(cle, GesteTel::personnage(p))          // Gabarit<_, (), Evenement>
Champ::new(objet, ContinuTel::brouillon(d))         // Gabarit<_, Texte<L>, Dernier>  ou Champs<L>
Croix::new(ContinuTel::aller())                     // Gabarit<_, Direction, Tenu>
Deplacables::new(zones, GesteTele::deposer())       // Gabarit<_, Depot<T, Z>, Evenement>
```

Un `Bouton` ne peut pas porter une action qui attend un texte, un `Champ` ne peut pas remplir une action d'un autre rôle, une `Croix` ne peut pas produire un événement : ces erreurs ne compilent pas.

**Ce que reçoit le jeu.**

```rust
pub struct Geste<A, P> {
    pub action: A,
    /// L'instant estimé du geste, recalé sur l'horloge de l'autorité et borné :
    /// jamais avant `recu - fenetre`, jamais après `recu` (SAL-129, REM-19).
    pub fait: Pas,
    pub recu: Pas,
    /// La vue sur laquelle le geste a été fait : son pas et sa phase.
    /// Remplis par l'autorité depuis ce qu'elle avait envoyé, jamais par l'appareil.
    pub vu: Vu<P>,
}
pub struct Vu<P> { pub pas: Pas, pub phase: P }

pub enum EvenementDe<J: Jeu> {
    Telephone { de: Appareil, geste: Geste<<J::Telephone as Role>::Evenement, J::Phase> },
    Tele { geste: Geste<<J::Tele as Role>::Evenement, J::Phase> },
    Manette { de: Appareil, bouton: BoutonManette, quoi: Appui, fait: Pas },
}

pub struct Continu<C> { pub valeur: C, pub fait: Pas, pub recu: Pas, pub cause: Cause }
pub enum Cause { Geste, Silence, Depart, PremierPlanPerdu, PauseDuCommun, TropVieux }

pub enum ContinuDe<J: Jeu> {
    Telephone { de: Appareil, continu: Continu<<J::Telephone as Role>::Continu> },
    Tele { continu: Continu<<J::Tele as Role>::Continu> },
    Manette { de: Appareil, axes: [Axes; 2], gachettes: [Borne<0, 1000>; 2] },
}
```

### 1.6 Le trait `Jeu`

```rust
pub trait Jeu: Etat {
    /// Où en est la partie. Sert à la péremption, à la progression et à l'épreuve.
    type Phase: Phase;
    type Telephone: Role;
    type Tele: Role;
    /// Ce qu'un refus dit à l'appareil qui a fait le geste.
    type Avis: Donnee + Affichable;
    /// L'identité d'une demande en cours (1.9, section 5).
    type CleDemande: Donnee + Eq;
    /// L'identité d'un effet ponctuel (section 5).
    type CleEffet: Donnee + Eq;
    /// Ce qu'une nouvelle partie reprend de l'ancienne, exprès.
    type Garde: Donnee;

    const DECLARATION: Declaration;

    fn creer(debut: Debut<'_, Self>) -> Self;
    fn garder(&self) -> Self::Garde;
    fn phase(&self) -> Self::Phase;

    /// Un événement. Peut refuser : le moteur rend alors la mémoire à l'identique.
    fn evenement(&mut self, e: EvenementDe<Self>, ctx: &mut Ctx<'_, Self>) -> Result<(), Arret<Self::Avis>>;
    /// Un état continu, ou son relâchement. Ne refuse pas : c'est un fait.
    fn continu(&mut self, c: ContinuDe<Self>, ctx: &mut Ctx<'_, Self>);
    /// Un pas de temps : à chaque pas si la cadence est continue, sinon à l'échéance.
    fn pas(&mut self, ctx: &mut Ctx<'_, Self>);
    /// Un appareil arrive, part, revient, ou change de capacités. Ne refuse pas.
    fn presence(&mut self, p: Presence, ctx: &mut Ctx<'_, Self>);
    /// La réponse à une demande encore en cours. Ne refuse pas.
    fn reponse(&mut self, cle: Self::CleDemande, r: Reponse, ctx: &mut Ctx<'_, Self>);

    /// Le prochain réveil voulu. Toutes les attentes du jeu y entrent : fin de phase,
    /// robots, grâce d'un absent.
    fn echeance(&self) -> Option<Echeance>;
    /// Qui la partie attend, pour que le commun vérifie qu'elle ne peut pas se figer.
    fn attente(&self) -> Attente;
    fn salle(&self) -> Ouverture;
    /// Les demandes en cours, recalculées depuis l'état.
    fn demandes(&self) -> impl Iterator<Item = (Self::CleDemande, Demande)>;
    /// L'avis d'un geste périmé, selon l'origine. `None` : ignoré sans message.
    fn avis_perime(origine: Origine) -> Option<Self::Avis>;

    fn tele(p: &Projection<'_, Self>) -> Ecran<Self>;
    fn telephone(p: &Projection<'_, Self>, moi: Appareil) -> Page<Self>;
}

pub trait Phase: Donnee + Clone + Eq {
    type Sorte: Enumerable;
    fn sorte(&self) -> Self::Sorte;
    fn terminale(&self) -> Terminale;      // Oui | Non
}

pub enum Arret<Av> { Refus(Option<Av>), SecretAbsent }
impl<Av> From<SecretAbsent> for Arret<Av> { /* … */ }
pub fn refus<Av>(avis: Av) -> Arret<Av>;

pub enum Attente {
    /// Une échéance existe : la partie avancera seule.
    Echeance(Echeance),
    /// Ces appareils doivent agir. Le commun vérifie que chacun est présent et que sa vue
    /// offre au moins un élément que le jeu accepterait, ou qu'une échéance existe (CAT-78).
    Appareils { qui: Vec<Appareil>, echeance: Option<Echeance> },
    /// L'organisateur doit agir : la vue de la télé doit offrir un élément accepté.
    Organisateur,
    /// Rien n'est attendu, et c'est voulu.
    Libre(Raison),
}
```

Les méthodes n'ont pas de corps par défaut. Un jeu qui n'a pas de téléphone écrit `type Telephone = Aucun;` et une méthode `telephone` dont le corps est `Page::sans_objet()`. La déclaration doit dire la même chose (1.10), et la compilation vérifie l'accord.

`Etat` (1.13) demande `Donnee + Clone`. L'état n'a aucune sérialisation écrite par le jeu : la dérivation la donne.

### 1.7 Le contexte

```rust
pub struct Ctx<'e, J: Jeu> { /* privé */ }
impl<'e, J: Jeu> Ctx<'e, J> {
    pub fn maintenant(&self) -> &Maintenant<'e>;
    /// Le temps jeté après un gel, au-delà du plafond de rattrapage. `None` d'habitude.
    pub fn saut(&self) -> Option<Duree>;
    /// Le générateur de la partie. C'est un secret : il manque sur un appareil, et un tirage
    /// arrête la prédiction de ce geste.
    pub fn hasard(&mut self) -> Result<&mut Hasard, SecretAbsent>;
    /// Un identifiant neuf, jamais redonné dans cette partie, même par une annulation du jeu.
    pub fn id(&mut self) -> Id;
    /// Oublie un appareil parti. Il revient en neuf, avec un autre `Appareil`.
    pub fn oublier(&mut self, a: Appareil) -> Result<(), PasParti>;
    /// Retire un appareil présent : il voit « Tu as été retiré », et revient en neuf.
    pub fn retirer(&mut self, a: Appareil);
    /// Demande une nouvelle partie : une instance neuve, avec `garder()` et les mêmes appareils.
    pub fn nouvelle_partie(&mut self);
}
```

Ce que le contexte fait est une sortie de l'entrée. Sur un refus, il est jeté avec le reste.

`Hasard` est un générateur à 256 bits d'état (ChaCha12), semé par une graine de 256 bits que fournit le commun, neuve à chaque soirée sauf rejeu voulu (PUN-89). Il vit dans la mémoire du module : un refus le rend à l'identique (HIS-44, PUN-27, OUT-171). Le rendu a son propre générateur, hors de l'état.

### 1.8 Les secrets

```rust
/// Connu de l'autorité seule.
pub struct Secret<T: Donnee>(Cache<T>);
impl<T: Donnee> Secret<T> {
    pub fn new(v: T) -> Secret<T>;
    pub fn lire(&self) -> Result<&T, SecretAbsent>;
    pub fn modifier(&mut self) -> Result<&mut T, SecretAbsent>;
    pub fn reveler(self) -> Result<T, SecretAbsent>;
}

/// Connu de l'autorité et de quelques appareils. La liste des lecteurs est elle-même secrète :
/// hors de ses lecteurs, un appareil ne sait même pas qui le connaît.
pub struct SecretDe<T: Donnee>(Cache<(Lecteurs, T)>);
impl<T: Donnee> SecretDe<T> {
    pub fn new(lecteurs: Lecteurs, v: T) -> SecretDe<T>;
    pub fn lire(&self) -> Result<&T, SecretAbsent>;
    pub fn lecteurs(&self) -> Result<&Lecteurs, SecretAbsent>;
    pub fn modifier(&mut self) -> Result<&mut T, SecretAbsent>;
    pub fn reveler(self) -> Result<T, SecretAbsent>;
}
pub struct Lecteurs(/* au plus 8 appareils */);
impl Lecteurs { pub fn un(a: Appareil) -> Lecteurs; pub fn aucun() -> Lecteurs; }

pub struct SecretAbsent;
```

`Debug` d'un secret écrit `<secret>`. La section 4 dit comment le lecteur est posé.

### 1.9 Vues, effets, demandes

Les types sont dans la section 5. En résumé :

```rust
pub enum Ecran<J: Jeu> { Scene(Scene<J>), Page(Page<J>) }   // la télé : scène GPU, ou page d'ordinateur
pub struct Page<J: Jeu> { /* arbre d'éléments rendu en DOM, gestes du rôle Telephone ou Tele */ }
pub struct Effet<K> { pub cle: K, pub instant: Pas, pub peremption: Duree, pub quoi: Ponctuel }
pub enum Demande { Image(..), Rendre(..), Lire(..), Fichier(..), Ligne(..), Temps(..) }
pub enum Reponse { Image(..), Rendu(..), Lu(..), Fichier(..), Ligne(..) }
```

### 1.10 La déclaration

```rust
pub struct Declaration {
    pub nom: Nom,
    pub salle: Salle,
    pub telephones: Usage<Telephones>,
    pub manettes: Usage<Manettes>,
    pub tele: Tele,
    pub temps: Temps,
    pub pause: Pause,
    pub prediction: Usage<Fenetre>,
    pub hasard: Usage<()>,
    pub secrets: Usage<()>,
    pub fin: Usage<Relance>,
    pub sauvegarde: Sauvegarde,
    pub capacites: Voulues,
    pub memoire_max: Octets,
    pub assets: &'static [Asset],
}

/// Pas d'`Option` : « sans objet » est une valeur, avec sa raison (HIS-04, OUT-03, OUT-04).
pub enum Usage<T> { Oui(T), SansObjet(Raison) }

/// Une raison qui dit quelque chose : non vide, au moins trois mots. Vérifiée à la compilation.
pub struct Raison(&'static str);
impl Raison { pub const fn new(s: &'static str) -> Raison; }

pub struct Salle { min: u16, max: u16, appareils_max: u16 }
impl Salle {
    /// Refuse à la compilation une salle incohérente : min > max, max = 0,
    /// plus de joueurs que d'appareils possibles (OUT-200, HIS-13).
    pub const fn joueurs(min: u16, max: u16) -> Salle;
    pub const fn appareils(self, max: u16) -> Salle;
}

pub enum Telephones { UnParJoueur, UnParEquipe, UnPourLaPartie }
pub enum Tele { Scene { format: Format, zone_sure: Pourcent }, Page }

pub struct Temps {
    pub cadence: Cadence,            // Continue (un pas à chaque pas) | Echeances
    pub rattrapage: Duree,           // au-delà, le temps est jeté et `ctx.saut()` le dit
    pub fenetre: Duree,              // replacement des gestes en retard ; 0 = aucun
}
pub enum Pause { DuJeu, DuCommun, Aucune(Raison) }
pub enum Relance { Rejouer { hasard: Suite } }    // ce que la nouvelle partie reprend du générateur
pub enum Suite { Continue, Neuve }
pub enum Sauvegarde { Memoire, Historique { etapes: u16 } }
```

Chaque champ est obligatoire. Quand une version du commun ajoute une question, elle ajoute un champ : tous les jeux cessent de compiler jusqu'à ce qu'ils y répondent. Rien ne devient sans objet en silence.

La crate du module (1.14) vérifie l'accord entre la déclaration et les types, dans un `const` :
- `Telephone = Aucun` si et seulement si `telephones` vaut `SansObjet` ;
- l'état contient un `Secret` ou un `SecretDe` (lu dans `SCHEMA`) si et seulement si `secrets` vaut `Oui` ;
- un variant `#[predit]` existe si et seulement si `prediction` vaut `Oui` ;
- `Phase::Sorte` a au moins une valeur terminale si et seulement si `fin` vaut `Oui`.

Un désaccord est une erreur de compilation.

`capacites` fait exception à la forme `Usage<T>` : une capacité voulue ajoute un enrichissement, elle ne retire aucune obligation, puisque le jeu doit rester jouable sans elle. L'omettre ne dispense de rien. Le hasard ne se lit pas dans les types : un jeu qui déclare `hasard: SansObjet` et appelle `ctx.hasard()` commet une faute, signalée au premier appel.

### 1.11 Ce que le jeu fournit à l'épreuve

```rust
pub trait Bots: Jeu {
    /// La mémoire d'un bot. Hors de l'état : un bot est un appareil, pas une règle.
    type Memoire: Clone + Default + 'static;
    /// Un bot de téléphone choisit depuis sa vue, comme un humain (section 6).
    fn telephone(vue: &PageVue<Self>, m: &mut Self::Memoire, b: &mut Bot<'_>) -> Choix;
    /// L'organisateur choisit depuis la vue de la télé.
    fn organisateur(vue: &EcranVue<Self>, m: &mut Self::Memoire, b: &mut Bot<'_>) -> Choix;
}

pub trait Invariants: Jeu {
    /// Ce qui doit être égal entre deux exécutions qui doivent avoir la même histoire.
    type Histoire: Donnee + Eq;
    /// Un écran de la télé ou du téléphone, pour la galerie des pires cas.
    type Ecran: Enumerable;

    fn invariants(&self, v: &mut Verif<'_>);
    fn transition(avant: &Self, entree: &Applique<'_, Self>, apres: &Self, v: &mut Verif<'_>);
    fn histoire(&self) -> Self::Histoire;
    fn ecran(&self) -> Self::Ecran;
    /// Le pire cas de chaque écran, construit depuis les limites. Le `match` est exhaustif :
    /// un écran ajouté sans pire cas ne compile pas.
    fn pire_cas(e: Self::Ecran) -> Self;
    fn proprietes() -> Proprietes<Self>;
}
```

La section 6 détaille `Verif`, `Choix`, `Proprietes`.

### 1.12 Les gestes de l'ordinateur de la soirée

L'ordinateur de la soirée n'envoie pas de touches au jeu. Le jeu décrit, dans la vue de la télé, ce que chaque touche fait :

```rust
Raccourci::new(Touche::Espace, GesteTele::passer())
    .aussi(Touche::Entree)
    .repetition(Repetition::Ignoree)    // défaut ; `Permise` pour Ctrl+Z dans un éditeur
    .modificateurs(Mods::AUCUN);
```

Le commun écarte la répétition automatique du clavier, les renvois du réseau, et toute touche pressée pendant une composition (IME). Un raccourci cède au champ de texte qui a le focus : Cmd/Ctrl+Z appartient au champ pendant une saisie (TL-32, TL-34). Les touches du commun sont réservées et ne se déclarent pas : `D` pour le diagnostic, `F8` pour marquer un instant dans le journal (OUT-141, SAL-162).

Un jeu qui a besoin du clavier brut (un FPS) le déclare dans `capacites`, et reçoit des `EvenementDe::Clavier` typés, avec modificateurs et état de composition. Aucun de nos trois jeux n'en a besoin.

### 1.13 Versions et conversion

```rust
pub trait Etat: Donnee + Clone {
    /// L'état de la version précédente, ou `Neuf`.
    type Avant: Ancien;
    fn convertir(avant: Self::Avant) -> Result<Self, Incompatible>;
}
pub trait Ancien: Donnee {}
pub enum Neuf {}                         // aucune version antérieure
pub struct Incompatible(pub Raison);
```

L'ancien type est gardé tel quel dans le jeu, dans un module `versions::v1`. `soiree check` vérifie que son `SCHEMA` est celui qu'avait publié l'étiquette précédente : on ne peut pas modifier un ancien type sans que la vérification le voie. Une sauvegarde d'une version antérieure passe donc toujours par `convertir`, et jamais par un décodage qui « tombe juste » (HIS-51).

### 1.14 Les macros

| Macro | Où | Ce qu'elle fait |
|---|---|---|
| `#[derive(Donnee)]` | jeu | `serde`, `SCHEMA`, `visiter` ; refuse les attributs qui élargissent l'entrée et les champs qui ne sont pas `Donnee`. |
| `#[derive(Evenements)]`, `#[derive(Continus)]` | jeu | Gabarits, clés, sortes, péremption, prédiction, débit. Refuse une donnée qui n'est pas le dernier champ. |
| `#[derive(Enumerable)]` | jeu | `const TOUS: &[Self]`, pour les phases et les écrans. |
| `limite!` | jeu | Une limite de texte, unité et police comprises. |
| `texte!` | jeu | Un texte des sources, nettoyé et validé à la compilation. |
| `assets!` | jeu | Polices, images, sons : chemin, empreinte, table des glyphes d'une police. Des constantes typées (`PoliceId`, `ImageId`). |
| `contenu!` | jeu | Un fichier de contenu, lu et validé à la compilation : doublons, longueurs une fois les variables insérées à leur pire, accolades orphelines (PUN-55, CAT-104). |
| `jeu::module!(T)` | crate du module, générée | Les exports, la vérification `const` de la déclaration, l'allocateur, le gestionnaire de panique. |

`jeu::module!` est la seule macro qui écrit du code `unsafe` (des `#[unsafe(no_mangle)]` et la lecture du tampon d'échange). Elle vit dans une crate que l'outil génère, pour que la crate du jeu reste `#![forbid(unsafe_code)]`.

---

## 2. Le contrat binaire du module

### 2.1 Ce que le module est

- Cible `wasm32-unknown-unknown`, Rust 1.99, édition 2024, `panic = "abort"`, LTO.
- **Aucun import.** Le moteur refuse un module qui importe quoi que ce soit.
- Une seule mémoire, exportée sous le nom `memory`. Le moteur pose son maximum au chargement (2.5).
- Aucune fonction de démarrage (`start`) qui fasse autre chose qu'initialiser l'allocateur.
- Refusés au chargement : mémoire partagée, atomiques, fils, SIMD relâché (non déterministe), `table.set` et `table.grow`, toute instruction hors de la liste du moteur. Ce que le module peut faire se constate sur ses octets, pas sur une promesse.

### 2.2 Les exports

```rust
// Générés par jeu::module!(T). Rien d'autre n'est exporté.
#[unsafe(no_mangle)] extern "C" fn soiree_tampon(taille: u32) -> u32;   // adresse d'un tampon d'au moins `taille` octets
#[unsafe(no_mangle)] extern "C" fn soiree_appel(taille: u32) -> u32;    // lit un `Appel`, écrit un `Retour`, rend sa taille
```

Deux fonctions. Le moteur écrit un `Appel` encodé dans le tampon, appelle `soiree_appel`, puis lit le `Retour` au même endroit. Toute la richesse est dans les types `Appel` et `Retour` de `soiree-abi`, qui se versionnent avec le reste.

Sections personnalisées, écrites par l'outil après la compilation :

| Section | Contenu |
|---|---|
| `soiree.abi` | La version du contrat binaire, un `u32`. |
| `soiree.declaration` | La `Declaration` encodée, identique à ce que rend `Appel::Decrire`. Le moteur la lit avant d'instancier, et refuse sans exécuter. |
| `soiree.schemas` | Les `SCHEMA` de l'état, des gestes, des vues, des avis. |
| `soiree.sources` | L'empreinte des sources du jeu et du `Cargo.lock` (OUT-37). |

La **version du jeu** est l'empreinte BLAKE3 du module tel que l'outil l'a produit, avant toute réécriture. Elle se calcule, elle ne s'écrit pas. Une page ou une projection d'une autre version est refusée au lieu d'être décodée de travers (SAL-41, REM-77).

### 2.3 Les messages

```rust
// soiree-abi : une définition, utilisée par le moteur et par la crate jeu.
pub const ABI: u32 = 1;

pub enum Appel {
    Decrire,
    Creer { graine: [u8; 32], garde: Option<Vec<u8>>, appareils: Vec<PresenceFil> },
    Entree { numero: u64, entree: EntreeFil },
    Vues { lecteurs: Vec<LecteurFil> },
    Projeter { lecteur: LecteurFil },
    Charger { projection: Vec<u8> },                  // sur un appareil
    Predire { entree: EntreeFil },                    // sur un appareil
    Etat,                                             // échéance, attente, salle, phase
    Demandes,
    Garder,
    Exporter,                                         // l'état en postcard canonique
    Importer { schema: [u8; 32], octets: Vec<u8> },   // d'une version antérieure
    Bot { role: RoleFil, vue: Vec<u8>, memoire: Vec<u8>, graine: [u8; 32], maintenant: u64 },
    Verifier,                                         // invariants
    Transition { entree: EntreeFil },                 // applique et vérifie avant/après
    Histoire,
    Ecran,
    PireCas { ecran: u32 },
    Memoire,                                          // octets vivants de l'allocateur
}

pub enum EntreeFil {
    Evenement { origine: OrigineFil, action: Vec<u8>, fait: u64, recu: u64, vu_pas: u64, vu_phase: Vec<u8> },
    Continu { origine: OrigineFil, valeur: Vec<u8>, fait: u64, recu: u64, cause: CauseFil },
    Pas { maintenant: u64, saut: u64 },
    Presence(PresenceFil),
    Reponse { cle: Vec<u8>, reponse: ReponseFil },
}

pub enum Retour {
    Decrit(DeclarationFil),
    Issue(IssueFil),
    Vues(Vec<VueFil>),
    Octets(Vec<u8>),
    Etat { phase: Vec<u8>, terminale: u8, echeance: Option<u64>, attente: AttenteFil, salle: OuvertureFil },
    Demandes(Vec<(Vec<u8>, DemandeFil)>),
    Choix(ChoixFil, Vec<u8>),
    Rapport(RapportFil),
    Illisible(RaisonFil),          // le module n'a pas su décoder l'appel : panne de l'outil
}

pub struct IssueFil { pub statut: StatutFil, pub sorties: Vec<SortieFil> }
pub enum StatutFil { Accepte, Refuse { avis: Option<Vec<u8>> }, Perime { avis: Option<Vec<u8>> }, SecretAbsent }
pub enum SortieFil { Oublier(u64), Retirer(u64), NouvellePartie }
```

Le moteur ne connaît aucun type du jeu. Les actions, les phases, les avis, les clés et les projections sont pour lui des octets opaques, qu'il garde, compare et renvoie. Les types qu'il comprend (vues, effets, demandes, déclaration) sont ceux de `soiree-abi`. Dans `jeu`, `Scene<J>`, `Page<J>` et `Demande` sont des façades typées de ces types, avec le même encodage ; un test de `soiree-abi` compare leurs `SCHEMA` (SAL-89, SAL-90).

### 2.4 La sérialisation

- Le format est celui de **postcard 1.x** : entiers en varint, variants numérotés, longueurs préfixées, aucune étiquette.
- Le décodage consomme tout le cadre : un octet restant est un refus (REM-59, OUT-146, SAL-118).
- Une longueur annoncée plus grande que le reste du cadre échoue avant toute allocation (REM-58).
- Un `Entrant` se valide pendant le décodage (`serde(try_from)`) : un `Fini` NaN, une `Borne` hors bornes ou un `Texte` trop long ne se construisent pas.
- Ce qui doit être canonique (un fichier de sauvegarde) est relu, réencodé et comparé octet à octet (TL-11, TL-12).

### 2.5 Ce que le moteur ajoute au chargement

Le moteur réécrit le module avant de l'instancier. Les mêmes réécritures servent l'autorité, les appareils et l'épreuve.

| Réécriture | Pourquoi |
|---|---|
| **Compteur d'instructions.** Une globale `i64`, décrémentée à l'entrée de chaque fonction et de chaque boucle, du poids de son bloc. À zéro : `unreachable`. | Toujours rendre la main. Le compte est en instructions, pas en temps : un pas interrompu l'est aussi au rejeu, et une entrée isolée le reste. |
| **NaN canoniques.** Après chaque opération flottante qui peut produire un NaN, le résultat NaN est remplacé par le NaN canonique. | Seuls les bits d'un NaN diffèrent entre moteurs WebAssembly. Le reste des flottants est déterministe. |
| **Mémoire bornée.** Le maximum de la mémoire devient `memoire_max`. | Un `memory.grow` refusé devient une erreur d'allocation, donc une faute du jeu, jamais une machine à genoux. |
| **Globales exportées.** Chaque globale mutable (le pointeur de pile en premier) est exportée sous un nom réservé. | Copier et restaurer l'état entier : mémoire et globales. |

Ce que le moteur refuse au chargement, il le dit avec le nom de l'instruction ou de l'import. Le rapport de l'épreuve contient la liste de ce que le module fait (accès, taille, maximum de mémoire) : c'est le constat que demande le contrat.

### 2.6 L'intérieur de la crate du module

`jeu::module!(T)` écrit, en plus des deux exports :
- l'**allocateur** global, déterministe, qui compte ses octets vivants (`Appel::Memoire`, OUT-84) ;
- le **gestionnaire de panique** : il écrit le message dans une zone fixe de la mémoire, puis exécute `unreachable`. Le moteur lit le message avant de restaurer la mémoire ;
- l'**enveloppe** `Partie<T>`, qui tient l'état du jeu, le générateur (dans un `Secret`), le compteur d'identifiants, le pas courant, et le lecteur en cours.

Tout cela vit dans la mémoire du module. Le copier, c'est copier la partie entière.

### 2.7 Les versions

| Ce qui change | Ce qui se passe |
|---|---|
| `ABI` | Le moteur accepte une plage de versions. Hors de la plage : refus au chargement, avec les deux numéros. |
| Le module (version du jeu) | Une page, une projection ou un instantané d'une autre version est refusé. Un appareil reçoit toujours le module de l'autorité (CONTRAT, « Le relais reste générique »). |
| Le schéma de l'état | Un instantané mémoire ne vaut que pour sa version. Changer de version passe par `Exporter` sur l'ancien module, puis `Importer` sur le nouveau, qui décode `Etat::Avant` et appelle `convertir`. |

L'ancien module est gardé avec la sauvegarde, par son empreinte. Un instantané ne se lit jamais sans le module qui l'a produit.

Le Component Model viendra plus tard. Le passage remplacera les deux exports et le tampon par une interface WIT générée depuis `soiree-abi`, sans changer les traits de `jeu`. C'est la raison de ne mettre aucun type du jeu dans `soiree-abi`.

---

## 3. Le moteur

### 3.1 Une entrée

Chaque entrée passe par le même chemin, sur l'autorité :

```
pour chaque entrée e, dans l'ordre fixé (3.2) :
  1. valider e à la frontière : vue et élément connus, donnée du bon type, dans sa plage
     (sinon : refus typé à l'appareil, rien n'atteint le module)                      SAL-117, SAL-152
  2. écrire e dans le journal, avec son numéro                                         SAL-157
  3. si e est un événement : prendre un instantané S de la mémoire et des globales (3.3)
  4. poser le carburant au budget de cette sorte d'entrée
  5. Appel::Entree(e)
       Accepte       → jeter S ; exécuter les sorties ; relire Etat, Demandes, Vues
       Refuse        → restaurer S ; envoyer l'avis à l'appareil d'origine seulement
       Perime        → restaurer S ; avis_perime, ou rien
       SecretAbsent  → restaurer S ; faute du jeu : sur l'autorité, rien n'est absent
       piège         → restaurer S, ou reprendre (3.4) ; isoler e (3.5) ; rapport
  6. épreuve seulement : Verifier, et comparer la mémoire restaurée à celle d'avant
```

Après chaque entrée acceptée, le moteur relit l'état visible (`Appel::Etat`) et vérifie la loi « ne jamais figer » : si l'attente nomme des appareils sans échéance, chacun doit être présent et sa vue doit offrir au moins un élément d'événement (CAT-78). Pendant l'épreuve, il essaie ces éléments sur un instantané pour vérifier qu'au moins un est accepté.

Les vues sont recalculées après chaque entrée acceptée, mais envoyées seulement si leurs octets changent. Un compte à rebours ne change pas les octets : il est dessiné par le commun depuis l'échéance (PUN-50, SQ-93).

### 3.2 L'ordre

- Les entrées arrivent une à une. Le jeu ne voit jamais deux gestes à la fois.
- Dans la fenêtre déclarée (`temps.fenetre`), les gestes sont ordonnés par `fait`, pas par arrivée. Un geste en retard dans la fenêtre est replacé : le moteur revient à l'instantané d'avant son instant et rejoue. Au-delà, le pas est définitif.
- Deux gestes du même pas sont départagés par une permutation tirée de la graine, jamais par l'identité de l'appareil (CAT-27).
- Avec une fenêtre de zéro, l'ordre est celui des arrivées, et tout pas est définitif dès qu'il est joué. C'est le cas de nos trois jeux.

Un jeu qui veut une résolution simultanée (deux coups mortels du même pas) la fait lui-même : il accumule les gestes d'un pas dans son état, et les résout dans `pas`. L'ordre d'arrivée n'est pas une règle d'équité (CAT-28).

### 3.3 Copier, restaurer, comparer

- **Copier**, c'est copier la mémoire, les globales exportées et la liste des blocs nommés. La mémoire d'un de nos jeux est de l'ordre du Mo (à mesurer) ; les blocs ne se copient pas.
- **Restaurer**, c'est réécrire ces octets dans une instance, après `memory.grow` si besoin.
- **Comparer**, c'est comparer des empreintes BLAKE3 de la mémoire et des globales.

Le moteur ne prend un instantané qu'avant un événement : un pas de temps, un continu, une présence et une réponse ne refusent pas. Un piège pendant l'une d'elles passe par la reprise (3.4).

Une optimisation reste ouverte : marquer les pages modifiées par une réécriture des écritures en mémoire, pour ne copier que celles-là (section 9, R-03).

### 3.4 Reprendre après un plantage

Une seule façon de repartir, que tout emprunte : l'onglet qui se recharge, le module qui piège pendant un pas, l'ordinateur qui redémarre.

1. Le moteur garde un instantané récent et le journal qui le suit. Il les confie, chiffrés, au relais (CONTRAT, « La reprise survit à un onglet qui se recharge »).
2. Un instantané porte l'empreinte de son module, le numéro de la dernière entrée appliquée, et l'empreinte de la mémoire (SAL-158).
3. Pour repartir : instancier le module réécrit, restaurer l'instantané, rejouer le journal en sautant les entrées écartées.
4. À chaque instantané intermédiaire, l'empreinte rejouée doit égaler l'empreinte notée. Un écart est une faute de déterminisme, signalée avec la première entrée qui diverge.

Les appareils ne voient de la reprise qu'un lien qui revient. Leurs gestes en attente repartent derrière la poignée de main (REM-07, REM-09). Une projection et une vue neuves leur sont renvoyées : aucun appareil ne reste sans vue.

### 3.5 Couper un pas trop long, isoler une entrée fautive

Un piège (panique, carburant épuisé, allocation refusée) arrête l'entrée en cours. Le moteur :

1. lit le message de panique dans la mémoire, avant toute restauration ;
2. restaure l'instantané, ou reprend (3.4) ;
3. marque l'entrée **écartée** dans le journal, avec sa cause. Les rejeux la sautent : la reprise ne rejoue pas la même faute en boucle ;
4. envoie à l'appareil d'origine un refus typé `FauteDuJeu`, que le commun affiche ;
5. écrit un **rapport de faute** : la graine, l'instantané, l'entrée isolée, le carburant consommé, le message. `soiree rejouer <rapport>` refait la faute sur la machine du développeur (HIS-07, OUT-141).

Un pas de temps écarté n'est pas rejoué ; le suivant arrive à son heure. Trois pas de temps écartés de suite arrêtent la partie : la télé le dit, et le rapport part. Un jeu qui ne sait plus avancer ne peut pas être sauvé par le commun, mais il ne peut pas l'emporter avec lui.

Le rapport distingue les issues (HIS-06, OUT-11) :

| Issue | Exemple | Qui a échoué |
|---|---|---|
| Faute du jeu | panique, budget, mémoire, secret absent sur l'autorité, vue invalide, attente figée, invariant faux | le jeu |
| Panne de l'outil | `Retour::Illisible`, moteur qui ne sait pas restaurer, journal illisible | le commun |
| Refus | `Refuse`, `Perime` | personne : c'est une règle |

### 3.6 Le temps du moteur

- Le moteur tient l'horloge de l'autorité, sur une horloge monotone qui compte le sommeil quand le navigateur le permet, et le signale sinon (SAL-51).
- Il livre les pas un à un. Après un retard, il en rattrape au plus `temps.rattrapage`, puis jette le reste et le dit par `saut` (SQ-62).
- Il ne livre un `Pas` que si la cadence est continue ou si `echeance()` est atteinte. Un jeu au tour par tour ne coûte rien entre deux échéances.
- Si le commun possède la pause (`Pause::DuCommun`), il suspend les pas et relâche les continus tenus, avec la cause `PauseDuCommun`.

### 3.7 Comparer deux exécutions

L'épreuve fait tourner plusieurs exécutions en lockstep, avec le même journal :

| Comparaison | Ce qu'on compare | Ce que ça attrape |
|---|---|---|
| Module contre module | empreinte mémoire et `Retour` après chaque entrée | non-déterminisme qui aurait échappé à l'enfermement |
| Module contre copie native | `Retour` et `Exporter` après chaque entrée (les mémoires n'ont pas la même forme) | une règle d'écriture contournée dans la copie native |
| Identifiants permutés | `Retour` après remise des identifiants, et `histoire()` | une règle qui dépend de l'identité d'un appareil (CAT-27) |
| Avec et sans prédiction | `histoire()` à chaque pas définitif | une prédiction qui décide (CONTRAT, « Les lois communes ») |
| Avec et sans coupure courte | `histoire()` | une coupure qui change le résultat (PUN-73, SQ-28) |

La copie native exécute le même code d'enveloppe que le module, appelé par une fonction Rust au lieu d'un export : `fn appeler(&[u8]) -> Vec<u8>`. Elle restaure un refus en réécrivant l'état depuis `Exporter`, plus lentement.

---

## 4. Secrets, projection et prédiction

### 4.1 Le lecteur

L'enveloppe du module tient un **lecteur**, que seul le moteur pose, par l'appel qu'il fait :

| Appel | Lecteur |
|---|---|
| `Entree`, `Etat`, `Demandes`, `Verifier`… sur l'autorité | `Autorite` : tout se lit |
| `Vues` pour la télé | `Public` : aucun secret ne se lit |
| `Vues` pour un téléphone | `Appareil(a)` : les `SecretDe` dont `a` est lecteur |
| `Projeter { lecteur }` | celui qu'on projette |
| `Predire`, sur un appareil | `Appareil(a)` |

`Secret::lire` et `SecretDe::lire` consultent ce lecteur. Le jeu ne peut pas le poser : il n'existe pas d'API pour cela, et le jeu n'a pas de `static` à mutabilité intérieure (1.1).

`Jeu::tele` reçoit une `Projection` dont le lecteur est `Public`. La télé ne peut donc pas montrer un secret, bien qu'elle soit dessinée sur la machine qui les détient : le trou de la télé (CAT-46) se ferme par la forme. Une vue de téléphone ne lit que les secrets de son appareil (SQ-64).

### 4.2 La projection

`Projeter` encode l'état avec le lecteur posé : un secret lisible s'encode `Some(valeur)`, un autre `None`. Sur l'appareil, `Charger` décode cette projection comme un état ordinaire, dont les secrets absents sont vides. La projection se calcule, elle ne s'écrit pas.

Elle est envoyée comme un état : le dernier gagne, renvoyée en entier à chaque reconnexion, sans delta (SAL-113). Elle n'est envoyée qu'aux appareils qui prédisent. Un jeu sans prédiction n'envoie que des vues, et ses appareils n'ont pas besoin du module.

Ce que la forme ne voit pas : un jeu peut copier un secret dans un champ public. C'est alors une révélation explicite, écrite dans le code, et `soiree check` signale chaque `lire()` dont la valeur est écrite hors d'un secret (une analyse simple, par la dérivation). L'épreuve ajoute une pression : les vues des appareils, générées sur des parties jouées par les bots, ne doivent contenir aucun octet d'une valeur secrète non révélée (CAT-46).

### 4.3 La prédiction

Seuls les variants `#[predit]` se prédisent. Sur l'appareil :

1. le commun garde la dernière projection confirmée, et les gestes de cet appareil qu'elle ne contient pas encore ;
2. à chaque nouvelle projection, il `Charger`, puis `Predire` chacun de ces gestes, dans l'ordre ;
3. un `Predire` qui rend `SecretAbsent` arrête la prédiction de ce geste et des suivants : ce morceau attend la confirmation ;
4. la vue de l'appareil se calcule sur l'état prédit. Elle est marquée prédite.

Le hasard est un secret. Un geste dont l'effet dépend d'un tirage ne se prédit donc jamais, sans que le jeu ait à le dire : il n'y a pas de tirage à deviner.

L'autorité ne reçoit que des gestes, jamais des prédictions. Elle dit à chaque appareil, avec sa projection, le numéro du dernier geste qu'elle a appliqué ou refusé : l'appareil jette ce qui est confirmé. Une prédiction coupée laisse le jeu correct, seulement moins réactif.

Un effet prédit porte la même `CleEffet` que l'effet confirmé : il n'est joué qu'une fois. Un effet prédit que la confirmation ne produit pas est annulé s'il n'a pas encore été joué.

**Le définitif.** Un résultat ne s'annonce qu'une fois définitif (REM-126). L'élément `Definitif(…)` d'une vue n'est rendu que si la vue vient d'un état confirmé et définitif. Un buzzer peut s'allumer sous le doigt ; « premier au buzzer » attend l'autorité.

Aucun de nos trois jeux ne prédit. Sidequest et Tier list n'ont rien à prédire (la télé est sur l'autorité) ; Punchline est au tour par tour. Ce mécanisme est écrit pour le jeu qui viendra, et l'épreuve l'éprouve d'abord sur le modèle de jeu.

---

## 5. Vues, effets et demandes

### 5.1 Deux langages, un même contrat

```rust
pub enum Ecran<J: Jeu> {
    /// Une scène dessinée par le commun sur le GPU, dans un repère fixe.
    Scene(Scene<J>),
    /// Une page rendue en DOM par le commun, qui peut contenir des scènes.
    Page(Page<J>),
}
```

- **La scène** sert la télé d'un jeu de salon : un repère fixe (`Repere::scene(16, 9)` ou `Repere::pixels(1280, 760)`), mis à l'échelle par le commun dans la zone sûre déclarée, à l'échelle entière si le jeu la demande. Toutes les longueurs sont des parts du repère, jamais de la fenêtre (PUN-51, PUN-52).
- **La page** sert un téléphone, ou l'écran d'un jeu d'ordinateur comme Tier list : un document qui coule, qui peut défiler, avec de vrais champs du navigateur.

Les deux sont des arbres de données, calculés depuis l'état. Le commun les rend, avec le navigateur aujourd'hui et son propre code demain, sans que le jeu change (CONTRAT, « Interaction riche »).

Cela tranche une question du contrat : la page téléphone est décrite par le jeu, dans ce langage, et rendue par le commun. Le jeu choisit tout ce qu'elle montre. Il n'écrit pas de code qui touche au DOM. La section 9 (Q-01) dit ce que cela coûte, et la porte qu'on peut garder.

### 5.2 Les nombres de l'affichage

Une coordonnée est un `Fixe`, un `i32` en 1/256 d'unité du repère. Une couleur est un `Rgba` de quatre `u8`. Une opacité est un `Borne<0, 255>`. Une durée d'animation est une `Duree`. Il n'y a pas de flottant dans une vue : NaN et l'infini n'y sont pas représentables (REM-57).

Le jeu peut calculer en flottants et convertir : `Fixe::depuis(x)` sature et ramène NaN à zéro, et l'épreuve signale chaque saturation.

### 5.3 Les éléments

```rust
impl<J: Jeu> Scene<J> {
    pub fn new(repere: Repere) -> Scene<J>;
    pub fn fond(&mut self, f: Fond) -> &mut Self;
    pub fn rect(&mut self, cle: Cle, r: Rect, s: Style) -> &mut Self;
    pub fn image(&mut self, cle: Cle, img: Source, r: Rect, e: Echantillonnage) -> &mut Self;
    pub fn texte(&mut self, cle: Cle, t: TexteVu, boite: Boite) -> &mut Self;
    pub fn groupe(&mut self, cle: Cle, t: Transform, f: impl FnOnce(&mut Scene<J>)) -> &mut Self;
    pub fn bouton(&mut self, cle: Cle, g: Gabarit<Ev<J::Tele>, (), Evenement>, b: Boite, nom: NomAccessible) -> &mut Self;
    pub fn raccourci(&mut self, r: Raccourci<Ev<J::Tele>>) -> &mut Self;
    pub fn compte_a_rebours(&mut self, cle: Cle, c: CompteARebours, boite: Boite) -> &mut Self;
    pub fn qr(&mut self, cle: Cle, boite: Boite) -> &mut Self;            // l'adresse que les téléphones joignent vraiment
    pub fn definitif(&mut self, f: impl FnOnce(&mut Scene<J>)) -> &mut Self;
    pub fn effet(&mut self, e: Effet<J::CleEffet>) -> &mut Self;
    pub fn avis(&mut self, zone: Boite) -> &mut Self;                   // où le commun place l'avis du dernier refus
}
```

La page a les mêmes éléments d'affichage, une mise en page qui coule (`pile`, `rangee`, `grille`), et des composants :

| Composant | Donnée qu'il produit | Ce que le commun tient |
|---|---|---|
| `Bouton`, `BoutonTenu` | `()`, ou `Appui` | action à l'appui ou au relâchement, au choix du jeu ; capture du pointeur ; tous les chemins de relâchement (SQ-07, SQ-08) |
| `Champ<L>` (une ligne), `Zone<L>` (plusieurs) | `Texte<L>` | vrai champ du navigateur, 16 px au moins (PUN-67) ; contenu, curseur, sélection et composition gardés tant que l'**objet** existe ; limite comptée dans l'unité de `L` ; validation bloquée pendant une composition (PUN-70, TL-31 à TL-33) |
| `Formulaire<L, N>` | `Champs<L>` | plusieurs champs, touche « suivant » puis « envoyer », envoi d'un bloc |
| `EnPlace<L>` | `Texte<L>` | texte éditable dans la page (titre, étiquette) ; un éditeur vidé vaut la chaîne vide |
| `Croix`, `Stick` | `Direction`, `Axes` | continu tenu, rappel, relâchement sur `blur`, `visibilitychange`, `pagehide` ; jamais sur une coupure du réseau (REM-10 à REM-12) |
| `Pad` | `Axes` et `Appui` | pad natif si la capacité existe, croix de la page sinon, même gabarit |
| `Deplacables<T, Z>` | `Depot<T, Z>` | copie qui suit le pointeur, élément d'origine déplacé en direct à sa future place, zones nommées par le jeu, place calculée dans l'ordre de l'état sans l'élément tenu, défilement près des bords en px par seconde avec plafond au réveil, annulation par Échap ou `pointercancel` (TL-36 à TL-47) |
| `Selecteur<F>`, `DepotPartout<F>`, `Collage<F>` | `Fichiers<F>` | filtre, taille et type connus avant lecture, refus au-delà de la taille déclarée, page jamais remplacée par un fichier lâché, même fichier choisi deux fois (TL-14, TL-53, TL-54) |
| `Modale` | gabarit de fermeture | voile, clic sur le voile, Échap |
| `Lien` | aucune | un `Bloc` à télécharger, sous un nom |
| `Surface` | `Pointeur` | appui, déplacement, relâché, annulation, avec identifiant, bouton, position en unités du repère ; souris, doigt et stylet par un même chemin ; pas de défilement natif sous le doigt (TL-36) |
| `Balayage` | `()` | un balayage horizontal au-delà d'un seuil |

Chaque élément a une `Cle`, unique parmi ses frères, que le jeu choisit. L'identité d'un élément est sa clé, jamais sa position (SAL-165). La clé d'un champ est l'**objet** qu'il édite (par exemple `(phase, duel)`) : un brouillon reste attaché à cet objet, jamais à un numéro local qui revient. Si un identifiant peut revenir avec un autre contenu, comme après une annulation dans Tier list, la clé porte aussi ce qui distingue le contenu (TL-23).

Les ressources d'une page (écouteurs, capteurs, minuteurs, boucles) appartiennent au rendu du commun, et vivent tant que l'élément qui les demande existe. Elles sont rendues à la mort ou au remplacement de la page (REM-40, REM-45, SAL-163). Le jeu n'en tient aucune.

Un geste commencé finit quand la vue change : un glisser dont la zone disparaît est annulé, un bouton tenu dont l'élément disparaît est relâché, avec la cause (REM-23).

### 5.4 Les textes de l'affichage

```rust
pub struct TexteVu {
    contenu: TexteAffiche,      // jamais du balisage : un texte s'affiche, il ne s'interprète pas (TL-35)
    police: PoliceId,
    taille: Taille,             // Fixe(t) | Budget { max, min } : rétrécit pour tenir
    alignement: Alignement,
    lignes: Lignes,             // Une | AuPlus(n) | Libre
    coupe: Coupe,               // Mots | Graphemes ; le signe isolé reste avec son mot (SQ-46)
}
```

Les polices sont chargées avant toute mesure (PUN-57). Le commun mesure, coupe et rétrécit ; le jeu ne lit jamais une mesure. L'information va de l'état vers l'image, jamais dans l'autre sens (CONTRAT, « Le GPU »).

### 5.5 Glisser entre deux pas, animer

- Un élément marqué `.interpole()` glisse entre deux vues successives, par sa clé. Le commun garde la marge nécessaire au-delà d'un pas (SQ-56).
- Une animation est une donnée : `Anim::depuis(debut: Pas).etapes([(Duree::ms(800), …), (Duree::s(2), …), (Duree::s(3), …)])`. Elle se calcule depuis l'instant de départ, qui est dans l'état. Une vue renvoyée après une reconnexion reprend l'animation où elle en est, sans la recommencer (REM-12).
- `CompteARebours { echeance, format }`, ou `CompteARebours::fige(duree)` pendant une pause. Le commun le dessine depuis son estimation du pas de l'autorité, qui ne recule jamais, même après une reconnexion (PUN-39, SQ-36).

### 5.6 La validation de ce que le jeu décrit

Le moteur décode chaque vue comme une entrée venue de l'extérieur, parce que le module en est une :
- clés uniques parmi les frères, profondeur et nombre d'éléments bornés ;
- tout élément dans le repère, tout texte dans une police déclarée, toute image d'un asset ou d'un bloc connu ;
- taille encodée sous la limite d'un message de l'autorité, connue des deux côtés (SAL-121, SAL-122).

Une vue invalide est une faute du jeu. La précédente reste affichée, et le rapport dit pourquoi. Pendant l'épreuve, la galerie des pires cas (6.4) cherche ces fautes avant qu'un joueur ne les voie.

### 5.7 Les effets

```rust
pub struct Effet<K> {
    pub cle: K,               // identité sémantique : (partie, phase), (manche, joueur)…
    pub instant: Pas,         // quand il doit se produire, sur l'horloge de l'autorité
    pub peremption: Duree,    // au-delà, il ne se joue plus (défaut : 250 ms)
    pub quoi: Ponctuel,
}
pub enum Ponctuel { Son(SonId), Signal(Signal), Vibration(Vibration) }
```

Un effet est une donnée de la vue, calculée depuis l'état comme le reste. Le commun garde, par appareil, les clés déjà jouées. Il joue un effet si sa clé est neuve et s'il n'est pas périmé, en programmant le son à `instant` corrigé du retard de l'appareil (SAL-176). Conséquences :
- un réaffichage, un rejeu ou une reconnexion recalcule le même effet, avec la même clé : il n'est pas rejoué (PUN-66, SQ-79, SAL-167) ;
- un appareil absent, ou une page rechargée, voit un effet périmé : il ne le joue pas en retard (SAL-155) ;
- un effet ne peut pas se perdre dans une panne de l'autorité : l'état restauré le recalcule, et s'il n'est pas périmé, il se joue.

Le son n'est joué qu'avec un contexte audio vraiment `running` ; avant le premier geste, l'effet est compté comme perdu, et le diagnostic le dit (SAL-174, SAL-175). Une vibration vers un appareil sans la capacité est ignorée.

### 5.8 Les demandes

```rust
pub enum Demande {
    /// Décoder, recadrer au centre, réduire, encoder.
    Image { source: Bloc<Brut>, carre: u16, encodage: Encodage },
    /// Dessiner une scène hors écran, sans faire sauter d'image à l'écran (TL-57).
    Rendre { scene: SceneFil, echelle: u8 },
    /// Lire un fichier reçu comme une valeur de type T, de ce jeu, à ce schéma.
    Lire { fichier: Bloc<Brut>, schema: [u8; 32], taille_max: u64 },
    /// Écrire une valeur de ce jeu dans un fichier téléchargeable.
    Fichier { valeur: Vec<u8>, schema: [u8; 32] },
    /// Ajouter une ligne à un journal de résultats, exportable depuis l'ordinateur de la soirée.
    Ligne { journal: NomJournal, ligne: Vec<u8> },
}
impl Demande {
    pub fn lire<T: Donnee>(fichier: Bloc<Brut>, taille_max: u64) -> Demande;
    pub fn fichier<T: Donnee>(valeur: &T) -> Demande;
    pub fn ligne<T: Donnee>(journal: NomJournal, ligne: &T) -> Demande;
}

pub enum Reponse {
    Image(Result<Bloc<Image>, EchecImage>),
    Rendu(Result<Bloc<Png>, EchecRendu>),
    Lu(Lu),
    Fichier(Result<Bloc<Brut>, EchecFichier>),
    Ligne(Result<(), EchecEcriture>),
}
impl Lu {
    /// Vérifie que le schéma est celui de `T`, puis décode et valide.
    pub fn valeur<T: Donnee>(self) -> Result<T, RefusFichier>;
}
pub enum RefusFichier { TropGros, PasUneSauvegarde, AutreVersion(u32), Abime, Illisible }
```

**Les demandes en cours se déduisent de l'état.** `Jeu::demandes` les rend toutes, à chaque fois. Le commun compare à ce qu'il exécute déjà :
- une clé nouvelle est lancée ;
- une clé disparue est annulée ;
- une réponse dont la clé n'est plus dans `demandes()` est jetée avant d'atteindre le jeu.

Un Reset ou une restauration qui vide les demandes de l'état les annule toutes, et une réponse tardive ne touche à rien, par la forme (TL-44 à TL-46, SAL-164). Plusieurs demandes indépendantes à la fois, ou une seule qui en remplace une autre : c'est le choix des clés.

Une demande s'exécute au plus une fois par clé, même à travers une reprise : le commun note la clé avec la réponse. Une écriture n'est réputée faite que quand elle est vraiment terminée (TL-63). Un échec revient comme une réponse ; le jeu le dit, et la partie continue (SQ-78).

**Les fichiers.** `Demande::Fichier` écrit une enveloppe du commun : le nom du jeu, le schéma, la valeur en postcard canonique, et les blocs qu'elle nomme. `Demande::Lire` refuse dans l'ordre : trop gros (vérifié avant de lire, TL-14), pas une enveloppe de ce jeu, autre version (lu avant la forme, TL-13), abîmé (relu, réencodé et comparé, TL-11). Le jeu n'écrit aucun sérialiseur ; il valide ensuite le sens (une tuile placée deux fois, TL-07).

---

## 6. Bots, invariants, et l'épreuve

### 6.1 Les bots jouent ce que l'écran demande

```rust
pub struct PageVue<J: Jeu> { /* la page telle que l'appareil l'a reçue, éléments compris */ }
impl<J: Jeu> PageVue<J> {
    pub fn boutons(&self) -> impl Iterator<Item = ElementVu<'_, ()>>;
    pub fn champs<L: Limite>(&self) -> impl Iterator<Item = ElementVu<'_, Texte<L>>>;
    pub fn formulaires<L: Limite>(&self) -> impl Iterator<Item = ElementVu<'_, Champs<L>>>;
    pub fn croix(&self) -> Option<ElementVu<'_, Direction>>;
    pub fn par_cle(&self, cle: &Cle) -> Option<ElementVuQuelconque<'_>>;
    pub fn action<D: Entrant>(&self, e: &ElementVu<'_, D>) -> Option<&Ev<J::Telephone>>;  // ce que l'élément enverrait
}
impl<D: Entrant> ElementVu<'_, D> {
    pub fn activer(&self, donnee: D) -> Choix;        // la donnée a le type que l'élément annonce
}

pub enum Choix {
    Rien,
    Attendre(Duree),
    Activer(Activation),
    Couper(Duree),          // le lien tombe, sans fermeture : comme un Wi-Fi mort (HIS-14)
    Silence(Duree),         // l'appareil se tait, page vivante
    Recharger,
    Jumeau,                 // un second onglet prend la place
    Partir,
}

pub struct Bot<'b> { pub moi: Appareil, pub hasard: &'b mut HasardBot, pub depuis_vue: Duree /* … */ }
```

Un bot ne voit que sa vue, comme un humain. Il ne peut pas tricher avec un secret, et il ne peut jouer que ce que l'écran offre. S'il « ne trouve rien à faire » alors que la partie l'attend, c'est l'écran qui est faux. Ses gestes passent par le même chemin que ceux d'un vrai appareil : validation de l'élément, file, transport simulé (SAL-10).

Le commun fournit en plus un **bot hasardeux** générique, pour tous les jeux : il touche des éléments au hasard, tape des textes hostiles du corpus du commun (familles emoji, zalgo, contrôles de direction, balisage, `{joueur}`, très longs textes), lâche des fichiers abîmés, annule, se coupe, se recharge. Il est la pression « entrées absurdes » de l'épreuve.

### 6.2 Les invariants sont du code

```rust
pub struct Verif<'v> { /* privé */ }
impl Verif<'_> {
    pub fn vrai(&mut self, nom: &'static str, condition: bool);
    pub fn egal<T: PartialEq + Debug>(&mut self, nom: &'static str, a: T, b: T);
    pub fn au_plus<T: PartialOrd + Debug>(&mut self, nom: &'static str, a: T, max: T);
}
```

- Les noms sont uniques ; le rapport compte chaque invariant évalué. Un invariant qui n'a jamais été évalué n'a pas réussi (OUT-01, HIS-01).
- **Un invariant qui n'observe rien est refusé** (HIS-02, OUT-06). L'épreuve perturbe des états par `Donnee::visiter` (un entier, une position, un élément de liste) ; chaque invariant doit être faux sur au moins une perturbation. Sinon, il ne regarde rien de l'état, et le rapport le dit.
- **Chaque détecteur doit avoir échoué une fois** (OUT-27, HIS-03). L'épreuve mute le code du jeu ; le rapport, sans seuil, dit quels mutants aucun invariant ni aucun test n'a vus (OUT-95, OUT-96).
- `transition` porte les propriétés qui relient deux états : « un vote de plus ne coûte jamais de points », « une relance ne retire jamais un point ».
- Un oracle de test ne partage aucun code avec la règle qu'il vérifie (PUN-74, CAT-121). C'est une règle d'écriture, vérifiée par l'outil sur les imports.

Le commun ajoute à tout jeu ses propres lois, vérifiées à chaque entrée, sans rien demander :

| Loi | Comment |
|---|---|
| Un refus ne change rien | mémoire restaurée comparée à celle d'avant |
| Flottants finis | `visiter` |
| Ne jamais figer | `attente` contre les vues (3.1) |
| Vues valides | 5.6 |
| Effets non rejoués | clés jouées, après réaffichage, rejeu et reconnexion forcés |
| Mémoire bornée | pente des octets vivants sur une soirée simulée (OUT-84, OUT-85, HIS-10) |
| Progression | la phase se lit dans l'état ; les bots atteignent une phase terminale, puis une nouvelle partie commence sans rien redémarrer (OUT-29, HIS-41) |

### 6.3 Les propriétés de panne

```rust
pub struct Proprietes<J: Invariants> {
    /// Une coupure courte ne change pas l'histoire.
    pub coupure_transparente: Usage<Coupure<J>>,
    /// Une partie rejouée avec des identifiants permutés a la même histoire.
    pub permutation: Usage<()>,
}
pub struct Coupure<J> {
    pub duree_max: Duree,
    /// Les hypothèses de la propriété, écrites comme du code, pas comme une phrase.
    pub hypotheses: fn(&J, Appareil) -> bool,
}
```

L'épreuve joue chaque partie deux fois : une fois sans panne, une fois avec une coupure de l'un des appareils, plus courte que `duree_max`, quand `hypotheses` est vraie à son début. Elle compare `histoire()`. Cette seule propriété a trouvé trois bugs d'un coup dans l'ancien Punchline (PUN-73).

### 6.4 La galerie des pires cas

`Invariants::Ecran` énumère les écrans du jeu. `pire_cas` en construit un état par écran, depuis les limites. L'épreuve :
- vérifie que `pire_cas(e).ecran() == e` et que ses invariants tiennent ;
- rend sa vue sous chaque format déclaré (pour Punchline : 1920 × 1080, 1280 × 1024, 2560 × 1080, 900 × 1200, 1024 × 600) ;
- mesure, dans le langage de dessin et non dans le DOM, les débordements de boîte, les sorties de repère, les chevauchements, la taille du texte (PUN-54, PUN-58, SQ-49, SQ-50) ;
- vérifie que chaque écran vu pendant les parties des bots a son pire cas, ce que le `match` exhaustif garantit déjà.

### 6.5 Le déroulé

1. **Compiler** depuis les sources, dans un clone neuf (REM-153).
2. **Constater** : aucun import, liste des instructions, mémoire maximale ; accord de la déclaration et des types (déjà fait à la compilation, revérifié sur le module).
3. **Examiner** : chaque question du contrat a sa réponse dans la déclaration, et chaque leçon des mécaniques déclarées a sa réponse dans le document du jeu.
4. **Chasser** : les bots du jeu et le bot hasardeux, au nombre maximal de joueurs déclaré, sur un réseau simulé qui a un débit et un tampon (SAL-181), avec les pannes du catalogue mêlées : coupure, silence, trou noir, paquet retenu puis livré, redémarrage de l'autorité, rechargement, onglet jumeau (SAL-180, OUT-77). Chaque panne injectée est constatée avant qu'on juge la réaction (OUT-26). À chaque entrée : les lois du commun et les invariants du jeu. Les comparaisons de 3.7.
5. **Injecter des bugs** dans le code du jeu.
6. **Revérifier** les preuves que le jeu apporte.
7. **Mesurer** les budgets.

Le temps et le hasard étant des entrées, une soirée entière se joue à vitesse machine, sans navigateur ni réseau réels (CAT-112, OUT-83). Une soirée simulée de six heures compte la mémoire vivante, et le nombre d'appareils retenus pour le nombre de présents (PUN-15, SQ-76).

---

## 7. Les trois jeux

Esquisses. Elles montrent la forme, pas toutes les règles du cahier.

### 7.1 Punchline

```rust
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;
use alloc::vec::Vec;
use jeu::prelude::*;

jeu::assets! {
    police RUBIK = "web/fonts/rubik.woff2";
    police ROKKITT = "web/fonts/rokkitt.woff2";
    police SCHOOLBELL = "web/fonts/schoolbell.woff2";
}
jeu::limite! {
    pub Prenom = 12 capitales, police RUBIK;
    pub Reponse = 80 capitales, police SCHOOLBELL;
    pub ConsigneL = 100 capitales, police ROKKITT;
}
jeu::contenu! {
    // Refusé à la compilation : fichier vide, entrée trop longue une fois {joueur} remplacé
    // par 12 « M », accolades que rien ne remplace.
    CONSIGNES: [Consigne<ConsigneL>] = "content/prompts.fr.txt", variable joueur: Prenom;
    FINALES: [Consigne<ConsigneL>] = "content/finals.fr.txt", variable joueur: Prenom;
    SECOURS: [Texte<Reponse>] = "content/safety.fr.txt";
}

#[derive(Donnee, Clone)]
pub struct Punchline {
    n: u32,                                // numéro de transition : la phase le porte
    etape: Etape,
    joueurs: Vec<Joueur>,                  // ordre d'arrivée, robots compris
    sieges: Appareils<JoueurId>,
    absences: Appareils<Absence>,          // une horloge par personne (PUN-08)
    prochain: u32,                         // un identifiant de joueur ne resert jamais
    paquets: Secret<Paquets>,              // les consignes à venir
    manche: Option<Manche>,
    jamais_joue: Appareils<Pas>,           // téléphones sans prénom, oubliés 90 s après leur départ
}

#[derive(Donnee, Clone)]
enum Etape {
    Salon,
    Depart { fin: Echeance },
    Annonce { m: Num, fin: Echeance },
    Ecriture { m: Num, debut: Pas, fin: Echeance },
    Vote { m: Num, duel: u8, debut: Pas, fin: Echeance },
    Revelation { m: Num, duel: u8, debut: Pas, fin: Echeance },
    Scores { m: Num, fin: Echeance },
    Fin,
}

#[derive(Donnee, Clone)]
struct Duel {
    /// Pendant l'écriture, seuls les deux auteurs lisent la consigne.
    consigne: SecretDe<Texte<ConsigneL>>,
    /// Qui a écrit quoi : révélé à la révélation, jamais avant.
    auteurs: Secret<[Auteur; 2]>,
    /// Chaque auteur voit sa réponse (« C'est envoyé ! »), personne d'autre.
    reponses: [SecretDe<Option<Rendu>>; 2],
    /// Le brouillon ne s'affiche nulle part : connu de l'autorité seule.
    brouillons: [Secret<Option<Champs<Reponse>>>; 2],
    /// Copiées à l'ouverture du vote, sans auteur : publiques.
    au_vote: Option<(Texte<ConsigneL>, [Option<Rendu>; 2])>,
    /// Chaque votant voit son bulletin ; le compte est public, les choix non.
    bulletins: Appareils<SecretDe<Cote>>,
    revele: Option<Revelation>,
}

#[derive(Evenements)]
pub enum GesteTel {
    Prenom(#[donnee] Texte<Prenom>),
    Personnage(Perso),
    Lancer,
    AnnulerDepart,
    Envoyer { duel: u8, #[donnee] cases: Champs<Reponse> },
    JeSeche { duel: u8 },
    Voter(Cote),
    Rejouer,
    #[jamais_perime] FermerAvis,
}
#[derive(Continus)]
pub enum ContinuTel {
    #[dernier(debit = 4)]
    Brouillon { duel: u8, #[donnee] cases: Champs<Reponse> },
}
#[derive(Evenements)]
pub enum GesteTele { Lancer, Robot, Passer, Annuler, Rejouer }

pub struct Tel;  impl Role for Tel  { type Evenement = GesteTel;  type Continu = ContinuTel; }
pub struct Tele; impl Role for Tele { type Evenement = GesteTele; type Continu = Aucun; }

impl Jeu for Punchline {
    type Phase = Phase;                    // { n: u32, sorte: Sorte } ; Sorte::Fin est terminale
    type Telephone = Tel;
    type Tele = Tele;
    type Avis = Avis;                      // « Pas maintenant. », « Trop tard : l'écran a changé. »…
    type CleDemande = Aucun;               // rien à demander
    type CleEffet = (u32, Signal);         // un son par transition : il appartient à la phase
    type Garde = Garde;                    // joueurs, paquets ; le hasard continue

    const DECLARATION: Declaration = Declaration {
        nom: Nom::new("punchline"),
        salle: Salle::joueurs(3, 8).appareils(24),
        telephones: Usage::Oui(Telephones::UnParJoueur),
        manettes: Usage::SansObjet(Raison::new("les manettes n'ont aucun rôle dans ce jeu")),
        tele: Tele::Scene { format: Format::SEIZE_NEUVIEMES, zone_sure: Pourcent::new(90) },
        temps: Temps { cadence: Cadence::Echeances, rattrapage: Duree::pas(0), fenetre: Duree::pas(0) },
        pause: Pause::Aucune(Raison::new("une partie finit toujours, chaque phase a une échéance")),
        prediction: Usage::SansObjet(Raison::new("tour par tour, seuls les champs réagissent sous le doigt")),
        hasard: Usage::Oui(()),
        secrets: Usage::Oui(()),
        fin: Usage::Oui(Relance::Rejouer { hasard: Suite::Continue }),
        sauvegarde: Sauvegarde::Memoire,
        capacites: Voulues::AUCUNE.avec(Voulue::EcranAllume),
        memoire_max: Octets::mio(16),
        assets: ASSETS,
    };

    fn evenement(&mut self, e: EvenementDe<Self>, ctx: &mut Ctx<'_, Self>) -> Result<(), Arret<Avis>> {
        match e {
            EvenementDe::Telephone { de, geste } => match geste.action {
                GesteTel::Prenom(p) => self.rejoindre(de, p),
                GesteTel::JeSeche { duel } => {
                    let place = self.place(de, duel).ok_or(refus(Avis::PasMaintenant))?;
                    if self.en_finale() { return Err(refus(Avis::PasDeJoker)); }
                    // Les refus d'abord, le tirage ensuite. Même dans l'autre ordre, le moteur
                    // rendrait le générateur à l'identique (PUN-27) : la forme le garantit.
                    let h = ctx.hasard()?;
                    self.secourir(place, h)
                }
                GesteTel::Voter(cote) => self.voter(de, cote),
                // … chaque variant, sans `_ =>`
            },
            EvenementDe::Tele { geste } => match geste.action {
                GesteTele::Passer => { self.clore_phase(ctx); Ok(()) }
                // …
            },
            EvenementDe::Manette { .. } => Err(refus(Avis::PasMaintenant)),
        }
    }

    fn continu(&mut self, c: ContinuDe<Self>, _ctx: &mut Ctx<'_, Self>) {
        let ContinuDe::Telephone { de, continu } = c else { return };
        let ContinuTel::Brouillon { duel, cases } = continu.valeur;
        // Un brouillon arrivé après l'échéance ne compte pas (cahier 4.4).
        if self.ecriture_ouverte_a(continu.recu) { self.garder_brouillon(de, duel, cases); }
    }

    fn pas(&mut self, ctx: &mut Ctx<'_, Self>) {
        let m = ctx.maintenant();
        self.faire_jouer_les_robots(m);                // 1,5 s après le début de la phase, pas du réveil
        if self.fin_de_phase().is_some_and(|f| f.atteinte(m)) {
            self.clore_phase(ctx);                     // la phase suivante part de `m` (PUN-34)
        }
        self.liberer_les_sieges(m);                    // au salon, 90 s après le départ
    }

    fn echeance(&self) -> Option<Echeance> {
        // Fin de phase, robots, absents comptés dehors à 20 s, sièges à 90 s : le plus tôt.
        [self.fin_de_phase(), self.reveil_robots(), self.reveil_absences(), self.reveil_sieges()]
            .into_iter().flatten().min()
    }

    fn attente(&self) -> Attente {
        match &self.etape {
            Etape::Salon | Etape::Fin => Attente::Organisateur,
            Etape::Ecriture { fin, .. } | Etape::Vote { fin, .. } =>
                Attente::Appareils { qui: self.attendus(), echeance: Some(*fin) },
            Etape::Depart { fin } | Etape::Annonce { fin, .. }
            | Etape::Revelation { fin, .. } | Etape::Scores { fin, .. } => Attente::Echeance(*fin),
        }
    }

    fn tele(p: &Projection<'_, Self>) -> Ecran<Self> {
        let mut s = Scene::new(Repere::scene(16, 9));
        s.fond(Fond::tunnel(p.teinte()));
        match &p.etape {
            Etape::Vote { duel, fin, .. } => {
                let Some((consigne, reponses)) = p.duel(*duel).and_then(|d| d.au_vote.as_ref()) else {
                    return s.into();
                };
                s.compte_a_rebours(cle!("temps"), CompteARebours::vers(*fin).presse_sous(Duree::s(10)), HAUT_GAUCHE);
                s.texte(cle!("consigne"), TexteVu::new(consigne, ROKKITT).budget(48, 28), CONSIGNE);
                // Les auteurs sont un `Secret` : ici, `lire()` rend `SecretAbsent`. Rien à afficher.
                // …
            }
            // … chaque étape
        }
        s.raccourci(Raccourci::new(Touche::Espace, GesteTele::passer()).aussi(Touche::Entree));
        s.effet(Effet::son(p.signal(), (p.n, p.signal()), p.debut_phase()));
        s.into()
    }

    fn telephone(p: &Projection<'_, Self>, moi: Appareil) -> Page<Self> {
        let mut page = Page::new(Fond::pate(p.couleur_de(moi)));
        match (&p.etape, p.joueur_de(moi)) {
            (_, None) => {
                page.champ(Champ::new(cle!("prenom"), GesteTel::prenom())
                    .exemple("Léa").majuscules(Majuscules::Mots).touche_entree(Entree::Ok));
            }
            (Etape::Ecriture { .. }, Some(j)) => {
                if let Some((duel, consigne)) = p.ma_prochaine_consigne(moi) {
                    // L'objet édité est (transition, duel) : changer de consigne remplace les champs.
                    page.formulaire(Formulaire::<Reponse>::new(cle!((p.n, duel)), p.cases(), GesteTel::envoyer(duel))
                        .en_continu(ContinuTel::brouillon(duel)));
                    if !p.en_finale() { page.bouton(cle!("seche"), GesteTel::je_seche(duel), "Je sèche (½ points)"); }
                } else {
                    page.texte("C'est envoyé ! On attend les autres…");
                }
            }
            // …
        }
        page
    }

    fn avis_perime(o: Origine) -> Option<Avis> {
        match o { Origine::Telephone(_) => Some(Avis::TropTard), Origine::Tele => None }
    }
    // creer, garder, phase, presence, reponse, salle, demandes : sans surprise.
}
```

Ce que la forme tient ici : un brouillon ne s'affiche nulle part (`Secret`) ; un vote du duel précédent est périmé avant d'atteindre `voter` ; un « Je sèche » refusé ne tire rien ; la télé ne peut pas montrer un auteur avant la révélation ; un son de phase ne se rejoue pas ; une manche ne peut pas commencer sur une échéance d'avant le gel.

Ce que les invariants et les bots tiennent : les propriétés du partage des points (`transition`), le compteur qui ne lit jamais « 2/1 », le classement et les couronnes, la transparence d'une coupure de moins de 20 s (`Proprietes::coupure_transparente` avec `duree_max: Duree::s(20)`). Les bots répondent au hasard ou sèchent, votent après 1,5 s, et l'organisateur simulé lance, passe, puis rejoue.

### 7.2 Sidequest

```rust
#[derive(Donnee, Clone)]
pub struct Sidequest {
    etape: Etape,                          // Accueil, Salon, Question, Resultats, Fin
    pause: Option<Pas>,                    // début de la pause : sert à repousser l'échéance
    perso: Vec<Perso>,                     // ordre d'arrivée = ordre des identifiants de joueur
    de_qui: Appareils<JoueurId>,
    points: Appareils<u32>,                // les points appartiennent à l'appareil
    paquet: Paquet,
    curseur: Curseur,
    manche_suivante: u64,                  // continue à travers les parties
    resultats_a_ecrire: Vec<LigneResultat>,
    ecriture_echouee: Option<u64>,
}

#[derive(Donnee, Clone)]
struct Perso {
    id: JoueurId,
    appareil: Appareil,
    pos: [f32; 2],                         // flottants de l'état : vérifiés finis à chaque pas
    vit: [f32; 2],
    tenue: Axes,                           // la dernière direction reçue, relâchée par le commun
    en_ligne: EnLigne,
    dernier_saut: Option<Pas>,
    bulle: Option<(Texte<Message>, Pas)>,
    dernier_message: Option<Pas>,
}

#[derive(Evenements)]
pub enum GesteTel {
    Rejoindre { look: Look, #[donnee] prenom: Texte<Prenom> },
    Sauter,
    Message(#[donnee] Texte<Message>),
}
#[derive(Continus)]
pub enum ContinuTel {
    #[tenu(silence = 450 ms, age_max = 1 s)]
    Aller(#[donnee] Direction),
    #[tenu(silence = 450 ms, age_max = 1 s)]
    Stick(#[donnee] Axes),                 // le pad natif : même effet, autre donnée
}
#[derive(Evenements)]
pub enum GesteTele {
    Jouer, Suivant, Precedent, FinManche, Salon, Terminer, Rejouer,
    #[jamais_perime] Pause(Bascule),       // fixe une valeur : jamais périmée (SQ-23)
    #[jamais_perime] Chat(Bascule),
}

impl Jeu for Sidequest {
    type CleDemande = u64;                 // le numéro de manche d'une ligne à écrire
    type CleEffet = Vibre;                 // (manche, joueur) : une vibration de succès par point
    const DECLARATION: Declaration = Declaration {
        salle: Salle::joueurs(0, 71).appareils(71),
        temps: Temps { cadence: Cadence::Continue, rattrapage: Duree::pas(3), fenetre: Duree::pas(0) },
        pause: Pause::DuJeu,
        hasard: Usage::SansObjet(Raison::new("aucun tirage dans les règles, le code de salle vient du commun")),
        secrets: Usage::SansObjet(Raison::new("positions, zones et points sont publics par nature")),
        prediction: Usage::SansObjet(Raison::new("les téléphones ne montrent pas le monde et la télé fait foi")),
        tele: Tele::Scene { format: Format::pixels(1280, 760), zone_sure: Pourcent::new(90) },
        // …
    };

    fn continu(&mut self, c: ContinuDe<Self>, _ctx: &mut Ctx<'_, Self>) {
        let ContinuDe::Telephone { de, continu } = c else { return };
        let Some(p) = self.perso_de(de) else { return };
        // Le silence de 450 ms, l'âge d'1 s, le départ et la perte du premier plan arrivent ici
        // comme une valeur de repos, avec leur cause. Le jeu n'a pas d'horloge à tenir.
        p.tenue = match continu.valeur {
            ContinuTel::Aller(d) => Axes::depuis(d),
            ContinuTel::Stick(a) => a,
        };
    }

    fn pas(&mut self, ctx: &mut Ctx<'_, Self>) {
        let m = ctx.maintenant();
        if self.echeance_question().is_some_and(|e| e.atteinte(m)) { self.compter_manche(m); }
        if self.bouge() { physique::pas(&mut self.perso); }    // 4.2 du cahier, sqrt seulement
        self.supprimer_absents(m);                              // 90 s
    }

    fn demandes(&self) -> impl Iterator<Item = (u64, Demande)> {
        // Une ligne reste en cours tant que sa réponse n'est pas revenue : écrite au plus une fois.
        self.resultats_a_ecrire.iter().map(|l| (l.manche, Demande::ligne(NomJournal::RESULTATS, l)))
    }

    fn reponse(&mut self, manche: u64, r: Reponse, _ctx: &mut Ctx<'_, Self>) {
        self.resultats_a_ecrire.retain(|l| l.manche != manche);
        match r {
            Reponse::Ligne(Ok(())) => {}
            Reponse::Ligne(Err(_)) => self.ecriture_echouee = Some(manche),   // annoncé, jamais bloquant
            Reponse::Image(_) | Reponse::Rendu(_) | Reponse::Lu(_) | Reponse::Fichier(_) => {}
        }
    }

    fn tele(p: &Projection<'_, Self>) -> Ecran<Self> {
        let mut s = Scene::new(Repere::pixels(1280, 760).echelle_entiere_si(Pourcent::new(85)));
        s.image(cle!("salle"), Source::Asset(SALLE), Rect::plein(), Echantillonnage::PlusProche);
        for perso in p.en_ligne() {
            s.groupe(cle!(perso.id), Transform::position(perso.pos), |g| {
                g.image(cle!("sprite"), perso.sprite(), SPRITE, Echantillonnage::PlusProche);
            }).interpole();                        // glisse entre deux pas, par la clé du joueur
        }
        for v in p.vibrations_de_la_derniere_manche() {
            s.effet(Effet::vibration(Vibration::Succes, v.cle, v.instant).pour(v.appareil));
        }
        // Barre de l'organisateur : la seconde pression est un réglage du bouton, pas une règle.
        s.bouton(cle!("terminer"), GesteTele::terminer(), BARRE_TERMINER, "Terminer la partie")
            .confirmation(Confirmation::entre(Duree::ms(400), Duree::s(4), "TERMINER ?"));
        s.raccourci(Raccourci::new(Touche::Droite, GesteTele::suivant()));
        s.raccourci(Raccourci::new(Touche::E, GesteTele::terminer()).modificateurs(Mods::MAJ));
        s.into()
    }

    fn telephone(p: &Projection<'_, Self>, moi: Appareil) -> Page<Self> {
        let mut page = Page::new(Fond::uni(INK));
        let actif = p.commandes_actives();               // ni en pause, ni aux résultats, ni à la fin
        page.pad(Pad::new(ContinuTel::aller(), ContinuTel::stick())
            .saut(GesteTel::sauter(), Declenche::Appui)
            .actif(actif));                              // un appui est ignoré, un relâchement passe toujours
        page.champ(Champ::new(cle!("message"), GesteTel::message()).zone(3));
        page
    }
}
```

L'ordre des collisions est celui des identifiants de joueur, que le jeu crée lui-même dans l'ordre des arrivées : c'est une règle du jeu (SQ-71), et elle ne dépend pas de l'identité des appareils, que le jeu ne peut pas ordonner. La permutation des identifiants d'appareil (3.7) ne change rien.

Les relâchements forcés du cahier (pause, fin de manche, arrivée, retour) restent des règles du jeu, écrites dans l'étape : `p.tenue = Axes::ZERO`. Ceux qui viennent du monde (silence, départ, premier plan perdu, geste trop vieux) sont ceux du commun. La question du saut aux Résultats, ouverte dans le cahier, se tranche dans l'étape : `actif` règle la page, et `evenement` refuse ou accepte, une seule fois, pour la page comme pour le pad (SQ-26).

### 7.3 Tier list

```rust
#[derive(Donnee, Clone)]
pub struct TierList {
    plateau: Plateau,
    historique: Historique<Plateau, 200>,  // copies du plateau : les images sont des blocs partagés
    edition: Option<Edition>,              // le texte en cours : une seule étape d'annulation
    fenetre: Fenetre,                      // Fermee | Reglages(RangId) | Export(Option<Bloc<Png>>)
    imports: Vec<Import>,                  // demandes en cours, par identifiant
    restauration: Option<(Id, Bloc<Brut>)>,
    avis: Option<Avis>,
    generation: u64,                       // ne s'annule pas : distingue un identifiant rendu (TL-23)
}

#[derive(Donnee, Clone, PartialEq)]
pub struct Plateau {
    titre: Texte<Titre>,
    rangs: Vec<Rang>,
    reserve: Vec<TuileId>,
    contenus: BTreeMap<TuileId, Contenu>,  // Contenu::Image(Bloc<Image>) | Contenu::Nom(Texte<Nom>)
    prochaine_tuile: u32,
    prochain_rang: u32,
}

jeu::limite! {
    pub Titre = 500 utf16;
    pub Etiquette = 500 utf16;
    pub Nom = 60 utf16;
}

#[derive(Evenements)]
pub enum Geste {
    Deposer(#[donnee] Depot<TuileId, Zone>),        // Zone::Rang(RangId) | Reserve | Poubelle
    AjouterNom(#[donnee] Texte<Nom>),
    AjouterImages(#[donnee] Fichiers<Images>),
    Restaurer(#[donnee] Fichiers<Enveloppe>),
    FinEdition,
    Annuler, Retablir, Reset, Exporter, Sauvegarder,
    OuvrirReglages(RangId), Fermer,
    Couleur { rang: RangId, couleur: Borne<0, 14> },
    InsererRang(Position), SupprimerRang(RangId), ViderRang(RangId),
    Monter(RangId), Descendre(RangId),
    #[jamais_perime] EffacerAvis,
}
#[derive(Continus)]
pub enum Continu {
    #[dernier(debit = 30)] Titre(#[donnee] Texte<Titre>),
    #[dernier(debit = 30)] Etiquette { rang: RangId, #[donnee] texte: Texte<Etiquette> },
}

impl Jeu for TierList {
    type Telephone = Aucun;
    type CleDemande = Id;                  // de `ctx.id()` : jamais redonné, même par une annulation
    type CleEffet = Aucun;
    const DECLARATION: Declaration = Declaration {
        salle: Salle::joueurs(1, 1).appareils(0),
        telephones: Usage::SansObjet(Raison::new("une seule main manipule le plateau, par décision de l'hôte")),
        tele: Tele::Page,
        temps: Temps { cadence: Cadence::Echeances, rattrapage: Duree::pas(3), fenetre: Duree::pas(0) },
        hasard: Usage::SansObjet(Raison::new("un éditeur sans tirage ni mélange")),
        fin: Usage::SansObjet(Raison::new("un éditeur sans score ni fin de partie")),
        sauvegarde: Sauvegarde::Historique { etapes: 200 },
        // …
    };

    fn evenement(&mut self, e: EvenementDe<Self>, ctx: &mut Ctx<'_, Self>) -> Result<(), Arret<Avis>> {
        let EvenementDe::Tele { geste } = e else { return Err(Arret::Refus(None)) };
        match geste.action {
            Geste::Deposer(d) => self.agir(|p| p.deplacer(d.element, d.zone, d.indice)),
            Geste::AjouterImages(f) => {
                // Une demande par geste : toutes les images arrivent ensemble, en une seule étape.
                self.imports.push(Import { id: ctx.id(), fichiers: f.blocs() });
                Ok(())
            }
            Geste::Restaurer(f) => {
                let Some(fichier) = f.premier() else { return Err(Arret::Refus(None)) };
                self.restauration = Some((ctx.id(), fichier));
                Ok(())
            }
            Geste::Reset => {
                // Vider les demandes de l'état les annule toutes : une réponse tardive
                // ne touchera à rien (TL-44 à TL-46).
                self.imports.clear();
                self.restauration = None;
                self.fenetre = Fenetre::Fermee;
                self.agir(Plateau::reinitialiser)
            }
            // …
        }
    }

    fn demandes(&self) -> impl Iterator<Item = (Id, Demande)> {
        let imports = self.imports.iter().flat_map(|i| i.fichiers.iter().map(move |b|
            (i.id_de(b), Demande::Image { source: b.clone(), carre: 160, encodage: Encodage::WebpSinonPng(85) })));
        let restauration = self.restauration.iter().map(|(id, f)|
            (*id, Demande::lire::<Plateau>(f.clone(), Octets::mio(64).get())));
        let export = self.export_en_cours().map(|(id, scene)| (id, Demande::Rendre { scene, echelle: 2 }));
        imports.chain(restauration).chain(export)
    }

    fn reponse(&mut self, id: Id, r: Reponse, _ctx: &mut Ctx<'_, Self>) {
        match r {
            Reponse::Lu(lu) => match lu.valeur::<Plateau>() {
                Ok(p) if p == self.plateau => self.avis = Some(Avis::DejaOuvert),
                Ok(p) => match p.verifier() {      // une tuile placée deux fois, deux rangs de même identifiant
                    Ok(()) => { self.historique.pousser(&self.plateau); self.plateau = p; self.avis = Some(Avis::Restaure) }
                    Err(_) => self.avis = Some(Avis::Abime),
                },
                Err(refus) => self.avis = Some(Avis::depuis(refus)),   // trop gros, autre version, abîmé…
            },
            Reponse::Image(res) => self.recevoir_image(id, res),
            Reponse::Rendu(res) => self.recevoir_export(id, res),
            Reponse::Fichier(_) | Reponse::Ligne(_) => {}
        }
        self.restauration = None;
    }

    fn tele(p: &Projection<'_, Self>) -> Ecran<Self> {
        let mut page = Page::new(Fond::uni(PAGE)).largeur_max(1120).defile(Defile::Vertical);
        page.en_place(EnPlace::new(cle!("titre"), Continu::titre(), &p.plateau.titre).fin(Geste::fin_edition()));
        page.avis(p.avis.as_ref(), Geste::effacer_avis());   // à côté des avertissements du commun (TL-71)
        page.deplacables(Deplacables::new(Geste::deposer())
            .zones(p.plateau.rangs.iter().map(|r| ZoneVue::rang(r, p.cle_tuile())))
            .zone(ZoneVue::reserve(&p.plateau.reserve))
            .poubelle(Zone::Poubelle, "Drop here to remove")
            .seuil(Fixe::px(4)).opacite_copie(80).opacite_origine(20)
            .defilement(Defilement::bords(Fixe::px(70), VitesseMax::px_par_s(1320), Plafond::ms(50))));
        page.selecteur(Selecteur::<Images>::multiple(Geste::ajouter_images()).aussi_depot_partout().aussi_collage());
        page.raccourci(Raccourci::new(Touche::Z, Geste::annuler()).modificateurs(Mods::CMD_OU_CTRL)
            .repetition(Repetition::Permise));            // cède au champ qui a le focus (TL-34)
        if let Fenetre::Export(Some(png)) = &p.fenetre {
            page.modale(Modale::new(Geste::fermer()), |m| { m.lien(Lien::new("tierlist.png", png)); });
        }
        page.into()
    }
}
```

Le glisser, le défilement automatique et l'annulation par Échap sont au commun : la tuile tenue n'est pas dans l'état, et aucun pas de temps n'est nécessaire. Le jeu reçoit un seul `Deposer`, en termes de son état : zone et indice compté sans la tuile. Une restauration annule les imports par la forme. La sauvegarde ne demande aucun sérialiseur au jeu, et ce qu'il lit d'un fichier, il l'aurait écrit lui-même (TL-11).

La clé d'une tuile dans la vue est `(TuileId, empreinte du contenu)` : après « ajouter A, annuler, ajouter B », B a l'identifiant de A mais une autre clé, et le rendu ne garde rien de A (TL-23).

L'historique dans l'état survit au rechargement (le cahier le laissait à décider) : c'est un choix, que la déclaration `Sauvegarde::Historique` rend visible.

---

## 8. Ce que l'interface rend impossible

« Impossible » veut dire : la valeur fautive ne se construit pas, ou le programme fautif ne compile pas, ou le moteur l'empêche quoi que fasse le module. La dernière colonne dit lequel.

| Famille de bugs | Ce qui la supprime | Par | Leçons |
|---|---|---|---|
| Un refus qui consomme le hasard ou change l'état | Le moteur restaure la mémoire après tout refus ; le générateur y vit | moteur | HIS-44, PUN-27, OUT-171, SQ-25, TL-02 |
| Un état à moitié modifié après une panique | Même restauration, après un piège | moteur | SAL-140, HIS-49 |
| NaN, infini, sous-normal venus d'un appareil | `Entrant` n'a pas de flottant nu ; `Fini`, `Axes` valident au décodage | type | HIS-45, SQ-67, OUT-175, REM-63 |
| NaN dans une vue | Coordonnées en `Fixe` entier | type | REM-57 |
| NaN qui ne se rejoue pas au bit près | NaN canoniques à la réécriture | moteur | SQ-68 |
| Un téléphone qui envoie une commande d'organisateur | Rôles distincts ; le canal d'un téléphone ne décode que son rôle | type | SQ-21, SQ-22, PUN-48 |
| Une entrée pour une commande que l'écran n'affichait pas | L'appareil désigne un élément d'une vue envoyée ; l'action est celle du jeu | moteur et type | SAL-152 |
| Une action d'une phase passée | La phase de la vue est remplie par l'autorité, comparée avant l'étape | module | PUN-38, SQ-23, SAL-115, CAT-44 |
| Un index de duel périmé qui tue la salle | Péremption, plus `get()` sans panique, plus isolement de l'entrée fautive | module et moteur | PUN-79 |
| La télé qui montre un secret | `Jeu::tele` lit avec le lecteur `Public` | forme | CAT-46 |
| Un téléphone qui reçoit le secret d'un autre | Projection et vue calculées avec le lecteur de l'appareil | forme | SQ-64 |
| Une prédiction qui devine un tirage | Le générateur est un `Secret` | type | — (CONTRAT) |
| Une échéance chaînée sur l'échéance d'avant un gel | `Echeance` ne se crée que depuis `Maintenant` | type (oriente) | PUN-34 |
| Une échéance qui survit à sa partie | Une nouvelle partie est une instance neuve | moteur | CONTRAT, « La forme » |
| Un minuteur, une horloge, `Instant::now` | `no_std` ; le temps est une entrée | forme | OUT-145, PUN-92 |
| `HashMap` et son ordre | `no_std` ; `Donnee` ne l'implémente pas | forme | CONTRAT, « Règles d'écriture » |
| `sin`, `powf` qui diffèrent entre natif et module | absents de `core` | forme | HIS-50, OUT-144 |
| Une règle qui dépend de l'identité d'un appareil | `Appareil` n'est pas ordonné ; ordre d'insertion | type | CAT-27 |
| Une réponse à une demande annulée | Demandes déduites de l'état ; réponse filtrée par clé | module | TL-44, TL-45, TL-46, SAL-164 |
| Un son ou une vibration rejoués | Effets déduits de l'état, dédupliqués par clé, périmés | moteur | PUN-66, SQ-79, SAL-167, SAL-155 |
| Un texte trop long, coupé dans un graphème, invisible | `Texte<L>` nettoyé et coupé par le commun | type | TL-26, TL-28, PUN-23, PUN-24, CAT-83, CAT-85 |
| Une limite différente dans la page et dans les règles | `Limite` : une seule définition pour le champ et le type | type | TL-27, SQ-40 |
| Un prénom accepté qui ne s'affiche pas | La police déclarée filtre à l'entrée | type | SQ-38, SQ-39 |
| Un texte interprété comme balisage ou gabarit | `TexteAffiche` ; aucun interpréteur | forme | TL-35, SQ-43, CAT-87 |
| Un brouillon attaché à un numéro local | La clé d'un champ est l'objet édité | type | SAL-165, PUN-04 |
| Un réaffichage qui efface ce qu'on tape | Le champ appartient au rendu du commun, gardé par clé | forme | HIS-47, PUN-40, TL-33, OUT-177 |
| Une ressource de page jamais rendue | Les ressources appartiennent aux éléments du commun | forme | REM-40, REM-45, SAL-163 |
| Une exemption silencieuse | `Usage<T>` sans `Option`, champs obligatoires, `Raison` non vide | type | HIS-04, HIS-43, OUT-03, OUT-04, OUT-05 |
| Une configuration incohérente | `Salle::joueurs` en `const fn` ; accord déclaration et types en `const` | compilation | OUT-200, HIS-13 |
| Un désérialiseur trop tolérant, une étiquette interne | La dérivation refuse `untagged`, `tag`, `flatten`, `default` | compilation | REM-60, PUN-91, SAL-119 |
| Des octets restants acceptés | Décodage qui consomme tout le cadre | module | REM-59, OUT-146, SAL-118 |
| Une page d'une autre version qui décode de travers | Version = empreinte du module ; refus typé | moteur | SAL-41, REM-77, PUN-44 |
| Une sauvegarde d'une autre version lue comme si de rien n'était | Instantané lié à son module ; conversion par `Etat::Avant` | type | HIS-51, TL-13 |
| Un fichier venu d'ailleurs que le jeu n'aurait pas écrit | Enveloppe du commun, relue et réencodée | moteur | TL-11, TL-12, TL-14 |
| Une sérialisation de l'état écrite à la main | Dérivée ; l'état est la mémoire | forme | CONTRAT, « Sauvegardes » |
| Un message trop gros perdu en silence | Limites de l'autorité dans la déclaration ; vue invalide = faute typée | moteur | SAL-07, SAL-121, OUT-197 |
| Un pas qui ne rend jamais la main | Compteur d'instructions | moteur | CONTRAT, « Chaque pas a un budget » |
| La même faute rejouée en boucle par la reprise | Entrée écartée dans le journal | moteur | CONTRAT, « Une entrée qui fait échouer » |
| Un invariant ou une commande qui n'observe rien | Perturbation des états ; mutants | épreuve | HIS-02, OUT-06, OUT-27 |
| Une partie qui se fige | `attente` vérifiée contre les vues | moteur et épreuve | CAT-78, PUN-05 à PUN-07 |
| Une épreuve qui suppose la forme d'un jeu | Salle, fin, rôles, téléphones déclarés dans des types | type | HIS-13 |
| Un bot qui triche ou joue hors de l'écran | Les bots ne voient que leur vue, et passent par l'élément | forme | PUN-76 |
| Un appui plus court qu'un échantillon perdu | Enfoncement et relâchement sont deux événements | type | REM-16, REM-17, SQ-99 |
| Une direction vieille qui fait glisser un personnage | `age_max` ; un repos passe toujours | commun | SQ-02, REM-18, CAT-25 |
| Une coupure qui lève les doigts | Les relâchements ont une cause ; la coupure n'en est pas une | commun | REM-10, REM-11, OUT-218 |
| Un geste en cours sans fin quand la vue change | Le rendu du commun termine le geste de l'élément disparu | forme | REM-23 |
| Un résultat annoncé avant d'être définitif | Élément `Definitif` | type | REM-126 |
| Un lien, un jeton ou une session dans le jeu | Le jeu ne voit qu'`Appareil`, sans constructeur | type | SAL-14, SAL-15 |

Ce qui n'est **pas** impossible, et reste sous pression :
- recopier un secret dans un champ public (4.2) ;
- se tromper de règle : un score faux avec des types justes (invariants, mutants, oracles indépendants) ;
- abuser de `Echeance::repoussee` pour chaîner des phases ;
- une mise en page qui déborde (galerie des pires cas) ;
- une fuite de mémoire dans l'état (pente mesurée) ;
- un jeu qui n'oublie jamais personne (nombre d'appareils retenus mesuré).

---

## 9. Risques et questions ouvertes

### Risques

**R-01 · Le gabarit repose sur l'ordre des octets de postcard.** Reconstituer une action par concaténation suppose que la donnée est le dernier champ et que rien ne s'encode autrement. La dérivation le vérifie, et un test de propriété compare, pour chaque gabarit, la concaténation à l'encodage direct. Si postcard change de format, ce test casse avant tout le reste.

**R-02 · La page déclarative peut brider un jeu.** Le contrat dit que l'intérieur de la page appartient au jeu. Ici, il le décrit sans le coder. Un jeu qui veut une interaction que le langage n'a pas (un dessin au doigt, un clavier de piano) attend que le commun l'ajoute. Voir Q-01.

**R-03 · Le coût des instantanés.** Une copie de la mémoire avant chaque événement, et des instantanés périodiques. Rien n'est mesuré. Le marquage des pages modifiées demanderait une réécriture des écritures en mémoire ; il exige une seconde mémoire (multi-mémoire, WebAssembly 3.0), ou une zone réservée dans la mémoire du jeu. La disponibilité de la multi-mémoire dans Safari est à vérifier.

**R-04 · Le coût du compteur d'instructions et des NaN canoniques.** Les deux réécritures ralentissent le module. L'ordre de grandeur est inconnu ; un budget de pas qui échoue s'instrumente, il ne se relâche pas (SQ-61).

**R-05 · Les blocs hors de la mémoire.** Le contrat dit « la mémoire entière est son état ». Ici, l'état est la mémoire et les blocs qu'elle nomme. Les blocs sont immuables et nommés par leur empreinte : le déterminisme tient, et la copie reste exacte. Mais c'est une extension du contrat, à accepter ou refuser par le mainteneur.

**R-06 · Le lecteur est une variable de l'enveloppe.** La protection des secrets dans les vues repose sur une cellule interne à `jeu`, et sur l'interdiction des `static` à mutabilité intérieure dans le jeu. Un contournement par `unsafe` est exclu par `forbid(unsafe_code)` ; un contournement par une crate tierce qui garde un état global l'est par l'audit des dépendances. C'est une garantie de forme, pas de type au sens strict.

**R-07 · Le texte nettoyé par le commun.** Le nettoyage dépend de la version d'Unicode du commun. Le journal garde le texte nettoyé, donc un rejeu ne change pas. Mais deux versions du commun peuvent accepter des prénoms différents. Un changement de version d'Unicode est un changement de sens, à documenter dans `CHANGELOG.md` avec son test.

**R-08 · `Maintenant` oriente sans prouver.** `Echeance::repoussee` reste nécessaire pour une pause, et permet de chaîner. La pression est un gel injecté : au réveil, au plus une transition de phase par entrée, et une durée de phase complète après la reprise.

**R-09 · Le calcul des vues de 71 téléphones.** Sidequest recalcule les vues après chaque pas, à 60 Hz. Si la mesure montre un coût, le jeu pourra déclarer une clé de vue bon marché par appareil (`fn cle_vue(&self, a) -> K`), recalculée seulement quand elle change (SQ-93).

**R-10 · `Appareil` sans ordre.** Un jeu qui voudrait une table triée par appareil doit passer par `Appareils<V>`, de recherche linéaire. Pour 72 appareils, c'est négligeable ; au-delà, l'index interne reste privé.

**R-11 · Les effets déduits de l'état gardent de l'état.** Pour qu'une vibration « point gagné » existe dans la vue, l'état doit garder la dernière manche et ses gagnants. C'est le prix de « jamais rejoué, jamais perdu ». La mémoire bornée se mesure.

**R-12 · Une seule implémentation pour deux côtés.** Les mêmes types `soiree-abi` servent le moteur et la crate `jeu`. Un test entre deux exemplaires du même code n'éprouve pas ce que l'autre côté exige (SAL-88). Il faut une seconde lecture indépendante du format, écrite sans `soiree-abi`, qui décode des journaux réels (SAL-90).

**R-13 · L'app iPhone et la prédiction.** Sur iOS, seul WebKit exécute du code téléchargé à pleine vitesse (REM-81). Une app qui voudrait prédire devra passer par sa vue web. Aucun de nos jeux ne prédit aujourd'hui.

### Questions ouvertes

**Q-01 · Une page libre ?** Faut-il, en plus du langage de page, une porte pour un jeu qui écrit sa propre page, dans un second module enfermé, qui ne reçoit que sa vue et ne renvoie que des gabarits complétés ? La validation par élément y serait perdue. Proposition : non pour cette version ; un jeu qui en a besoin le demande, et le commun ajoute le composant.

**Q-02 · La fenêtre de replacement.** Zéro pour nos trois jeux. Pour un jeu d'action, la durée dépend du genre (CONTRAT, « Questions ouvertes »). Le mécanisme est décrit, pas éprouvé.

**Q-03 · Le pas global.** 60 pas par seconde pour tous les jeux. Un jeu de rythme voudrait plus fin ; un jeu au tour par tour s'en moque. Faut-il une fréquence déclarée par jeu ?

**Q-04 · Le format des sauvegardes de Tier list.** L'enveloppe du commun remplace le JSON de l'ancien jeu. Lire les fichiers au format 1 déjà téléchargés demanderait une conversion écrite par le jeu, hors de `Demande::Lire`. À décider.

**Q-05 · Les commandes sans interface de Sidequest.** Suivant, Précédent, question libre « depuis l'ordinateur de la soirée seulement, sans interface ». Une console du commun qui envoie des gestes du rôle `Tele` suffit-elle, ou faut-il un formulaire caché dans la vue ?

**Q-06 · Les résultats exportables.** Une `Demande::Ligne` écrit dans un journal du commun. Où vit ce journal, et sous quel format sort-il de l'ordinateur de la soirée ?

**Q-07 · Le code de salle.** Sidequest nomme ses résultats et ses enregistrements par un code de salle stable. Le commun le tire et le garde ; faut-il le donner au jeu dans `Debut`, ou le jeu n'a-t-il pas à le connaître ?

**Q-08 · Le GPU direct.** Le contrat permet à un jeu qui en a besoin de parler directement au GPU. Rien ici ne le décrit. Un élément `Scene::gpu(programme, données)` dont les données partagent leur définition avec le programme serait la forme naturelle.

**Q-09 · L'énoncé des preuves.** Le contrat veut que le commun puisse exiger une preuve, d'abord « ne jamais planter la partie » sur un seul jeu. Avec cette interface, l'énoncé porterait sur `evenement`, `pas`, `presence` et `reponse` : aucune panique, terminaison sous le budget. Faut-il l'écrire dès la première version des traits ?

**Q-10 · La résolution simultanée.** Un jeu qui veut évaluer plusieurs gestes sur l'état d'avant (CAT-28) les accumule dans son état. Faut-il que le commun livre plutôt un lot de gestes du même pas, avec un type qui l'impose ?

**Q-11 · Le nom de la crate.** `jeu` est court et clair, mais générique sur crates.io. Les crates ne sont pas publiées sur crates.io aujourd'hui ; si elles l'étaient, `soiree-jeu`.

**Q-12 · Ce qu'on construit en premier.** Proposition : `soiree-abi` et `jeu` sans vue (données, temps, gestes, secrets, déclaration), puis un moteur sans navigateur qui fait tourner le modèle de jeu et l'épreuve sur une horloge simulée. Les vues et le rendu viennent ensuite. La première pression, avant tout jeu : un refus ne change rien, et une coupure courte est transparente, sur le modèle.
