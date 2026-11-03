# Proposition : l'interface d'un jeu, vue depuis le temps réel

> Une proposition pour la première pièce de la reconstruction : la crate `jeu`, que chaque jeu utilise, et le contrat binaire entre le module d'un jeu et le moteur du commun. Elle suit [`MANIFESTE.md`](../MANIFESTE.md) et [`CONTRAT.md`](../CONTRAT.md). Quand elle les contredit, ce sont eux qui gagnent.
>
> Son angle : le temps réel et le monde. La prédiction par la même étape, les secrets dans les types, la fenêtre avant qu'un pas soit définitif, les effets avec leur identité et leur instant, les pas fixes et le glissement, les bots et les invariants. Elle cherche ce qui casserait Sidequest ou un FPS.
>
> Rien ici n'est mesuré. Chaque chiffre est un point de départ, à mesurer avant d'être promis.

## 0. En bref

1. **Un seul mécanisme pour repartir.** Restaurer une mémoire, puis rejouer des entrées. Il sert au replacement d'un geste en retard, à la reprise après une faute, au rechargement d'un onglet, à la prédiction sur un appareil et au rejeu d'un bug. C'est le mécanisme « éprouvé en permanence » du manifeste : il tourne à chaque image.
2. **Deux exécutions sur l'ordinateur de la soirée.** L'instance *définitive* n'avance que sur des pas devenus définitifs. L'instance *de tête* part d'une copie de la définitive et rejoue la fenêtre. Un jeu sans fenêtre n'a qu'une instance.
3. **Une seule fonction d'étape, en deux temps.** Le jeu lit l'état, puis accepte ou refuse. Il ne peut modifier l'état qu'après avoir accepté. Il ne peut refuser qu'avant. Le compilateur fait respecter « un refus ne change rien ».
4. **Le temps est un numéro de pas.** Le jeu ne voit jamais une heure ni une image. Une échéance est un numéro dans son état. Un gel ne saute aucun pas : le temps perdu est jeté.
5. **Les entrées disent quand et sur quoi.** Chaque geste porte où il a été placé, quand il a été fait, quand il a été reçu, et le pas que l'appareil voyait.
6. **Deux sortes d'entrées d'appareil.** Un événement compte une fois. Un état tenu est complet et le dernier gagne ; le commun gère sa fraîcheur, son âge et son relâchement. Un bouton dans un état tenu porte un compteur d'appuis, pour qu'aucun appui ne se perde.
7. **Un secret est un `Voile<T>`.** La vue ne peut pas le lire : seule l'étape reçoit le jeton qui ouvre un voile. La télé aussi regarde à travers son regard.
8. **Une vue mêle la tête et le définitif.** Ce qui est marqué `Definitif` vient de l'instance définitive. Un résultat ne s'annonce donc qu'une fois sûr.
9. **Un effet a une identité, un instant, une tenue et une péremption.** Un réaffichage, un rejeu ou un replacement ne le rejoue pas. Une demande ne part que de l'instance définitive.
10. **Le jeu dit qui il attend, où il en est, ce qui doit rester vrai, et comment se jouer seul.** L'épreuve s'en sert, et vérifie que chacune de ces réponses observe quelque chose.

---

## 1. L'API Rust de la crate `jeu`

### 1.1 Organisation

| Crate | Pour | Qui la compile |
|---|---|---|
| `jeu` | les types communs, le trait `Jeu`, les enveloppes de l'étape, la macro `jeu::module!` | le module du jeu, le moteur, les pages |
| `jeu-derive` | `#[derive(Etat)]`, `#[derive(Message)]`, `#[derive(Vue)]` | via `jeu` |
| `jeu-page` | ce qu'une page téléphone utilise : vue reçue, envoi typé, horloge estimée, état du lien | la page du jeu |
| `jeu-moteur` | le moteur, côté commun | le commun seul |

Le jeu écrit une crate de règles (le module) et, s'il a des téléphones, une crate de page. Les deux dépendent de la même crate de types du jeu : un renommage d'un côté casse l'autre.

Dépendances tierces choisies : `serde` et `postcard` pour le format, `unicode-segmentation` et `unicode-normalization` pour le texte. Chacune avec sa raison, à côté de la dépendance.

### 1.2 Le trait du jeu

```rust
use jeu::prelude::*;

/// Ce qu'un jeu est. L'état est le type qui implémente ce trait.
pub trait Jeu: Etat + Sized + 'static {
    /// Ce qui configure une partie à sa création (paquet, options).
    type Config: Message;
    /// Les commandes venues de l'écran de l'ordinateur de la soirée.
    type ActionEcran: Message;
    /// Les événements venus d'un téléphone : comptés une fois, dans l'ordre.
    type GesteTelephone: Message;
    /// L'état tenu d'un téléphone : le dernier gagne. `()` s'il n'y en a pas.
    type TenuTelephone: Tenu;
    /// Un refus, renvoyé au seul appareil qui a fait le geste.
    type Refus: Message;
    /// Ce que reçoit la page d'un téléphone.
    type VueTelephone: Vue;
    /// Où en est la partie. L'épreuve vérifie qu'elle les parcourt toutes.
    type Phase: Message + Ord + Copy;
    /// Ce qu'une nouvelle partie garde exprès de la précédente.
    type Garde: Etat;
    /// L'état de la version précédente, ou `jeu::Aucun`.
    type Precedent: Etat;

    fn declarer() -> Declaration<Self>;
    fn creer(c: Creation<Self>) -> Result<Self, Self::Refus>;
    fn appliquer(avant: Avant<'_, Self>, entree: Entree<Self>) -> Issue<Self>;

    fn vue_ecran(&self, r: &Regard, s: &mut Scene<Self::ActionEcran>);
    fn vue_telephone(&self, r: &Regard) -> Self::VueTelephone;

    fn phase(&self) -> Self::Phase;
    fn attente(&self, a: &mut Attente<Self>);
    fn garder(&self) -> Self::Garde;
    fn convertir(ancien: Self::Precedent) -> Result<Self, Self::Refus>;

    /// Ce geste peut-il s'afficher avant confirmation ? Par défaut, non.
    fn previsible(_g: &Self::GesteTelephone) -> Prevision { Prevision::Non }
    /// A-t-il besoin de pas maintenant ? Lu après chaque entrée acceptée.
    fn besoin_de_pas(&self) -> Besoin { Besoin::Oui }
}

/// Ce que le jeu donne à l'épreuve. Exigé : `jeu::module!` ne compile pas sans.
pub trait Epreuve: Jeu {
    fn invariants(&self, v: &mut Verif<'_>);
    fn transition(avant: &Self, entree: &Entree<Self>, apres: &Self, v: &mut Verif<'_>);
    fn bot_telephone(etat: &Self, vue: &Self::VueTelephone, b: &mut Bot<'_, Self>);
    fn bot_ecran(etat: &Self, b: &mut Bot<'_, Self>);
    fn pires_cas(p: Self::Phase, g: &mut Galerie<Self>);
}

// Dans la crate du module :
jeu::module!(Sidequest);
```

`jeu::module!` écrit les exports (§ 2), installe l'allocateur de la crate et vérifie à la compilation que le type implémente `Jeu` et `Epreuve`. Il n'y a pas d'autre porte.

Les méthodes `vue_*`, `phase`, `attente`, `garder`, et tout le trait `Epreuve`, reçoivent `&self`. Elles ne peuvent rien changer. Le moteur le constate pendant l'épreuve (§ 3.6).

### 1.3 L'étape en deux temps

C'est le cœur de l'interface. Une seule fonction reçoit toutes les entrées, une à une.

```rust
/// Avant la décision : lecture seule.
pub struct Avant<'a, J: Jeu> { /* privé */ }

impl<'a, J: Jeu> Avant<'a, J> {
    pub fn lire(&self) -> &J;
    pub fn autorite(&self) -> &Autorite;
    /// Accepter donne l'écriture. On ne peut plus refuser ensuite.
    pub fn accepter(self) -> Apres<'a, J>;
    /// Refuser : l'état ne change pas, le refus va au seul appareil du geste.
    pub fn refuser(self, r: J::Refus) -> Issue<J>;
    /// Rien à faire : l'état ne change pas, personne n'est prévenu.
    pub fn ignorer(self) -> Issue<J>;
    /// Un secret manque (sur un appareil qui prédit) : ce morceau attend.
    pub fn attendre(self, a: Absent) -> Issue<J>;
}

/// Après la décision : écriture.
pub struct Apres<'a, J: Jeu> { /* privé */ }

impl<'a, J: Jeu> core::ops::DerefMut for Apres<'a, J> { /* vers J */ }

impl<'a, J: Jeu> Apres<'a, J> {
    /// L'état, le jeton des secrets et les sorties, empruntés ensemble.
    pub fn parts(&mut self) -> (&mut J, &Autorite, &mut Sorties<J>);
    pub fn fin(self) -> Issue<J>;
    pub fn attendre(self, a: Absent) -> Issue<J>;
}

impl<J: Jeu> Sorties<J> {
    #[must_use]
    pub fn emettre(&mut self, e: Effet) -> Result<(), Depassement>;
    #[must_use]
    pub fn demander(&mut self, d: Demande) -> Result<IdDemande, Depassement>;
}
```

- `Issue<J>` n'a pas de constructeur public. On ne l'obtient qu'en refusant, en ignorant, en attendant ou en finissant.
- `refuser` consomme `Avant`. `accepter` aussi. Après `accepter`, il n'y a plus d'`Avant` : refuser devient impossible à écrire.
- Le hasard du jeu vit dans son état (`Hasard`, § 1.7). Tirer demande `&mut`. Avant d'accepter, le jeu n'a que `&J` : un refus ne peut pas consommer de hasard.
- Les effets et les demandes ne s'émettent qu'après acceptation. Un refus n'émet rien d'autre que son refus.
- `#[derive(Etat)]` n'est implémenté pour aucun type à mutabilité intérieure (`Cell`, `RefCell`, `OnceCell`, atomiques, `Mutex`). Un état qui en contient ne compile pas. Seul `Partage<T>` (un `Rc` immuable) écrit un compteur en lecture ; l'épreuve le surveille (§ 3.6).

Un refus de sa propre entrée de temps n'a pas de sens : `refuser` sur un `Pas` est une faute du jeu, signalée par l'épreuve. Pour « rien à faire à ce pas », on écrit `ignorer`. C'est aussi le chemin bon marché : un pas ignoré ne touche à rien.

### 1.4 Les entrées

```rust
pub enum Entree<J: Jeu> {
    /// Le temps : un pas de plus.
    Pas(Pas),
    /// Une commande de l'écran de l'ordinateur de la soirée.
    Ecran(DeEcran<J::ActionEcran>),
    /// Un événement d'un téléphone.
    Geste(Geste<J::GesteTelephone>),
    /// Le changement d'un état tenu d'un téléphone.
    Tenu(Changement<J::TenuTelephone>),
    /// Le changement de l'état d'une manette.
    Manette(Changement<Manette>),
    /// Un appareil arrive, part, revient, change de capacités.
    Presence(Presence),
    /// La réponse à une demande.
    Reponse(Reponse),
}

pub struct Pas { /* privé */ }
impl Pas {
    pub fn numero(&self) -> NumPas;
    /// L'heure murale de l'ordinateur de la soirée, enregistrée.
    /// Pour étiqueter une ligne de résultats, jamais pour décider.
    pub fn heure_murale(&self) -> HeureMurale;
}

pub struct Geste<G> { pub de: Appareil, pub geste: G, pub quand: Quand }

pub struct Changement<T: Tenu> {
    pub de: Appareil,
    pub cle: T::Cle,
    pub valeur: T,
    pub cause: CauseTenu,
    pub quand: Quand,
}

pub enum CauseTenu {
    /// L'appareil a envoyé cette valeur.
    Geste,
    /// Plus de nouvelles depuis la fraîcheur déclarée : relâché à `neutre()`.
    Silence,
    /// L'appareil est parti : relâché à `neutre()`.
    Depart,
}

/// Quand et sur quoi un geste a été fait.
pub struct Quand {
    /// Le pas où le commun l'applique. C'est l'entrée en cours.
    pub place: Instant,
    /// L'instant estimé du geste, sur l'horloge de la partie. Borné :
    /// jamais plus tôt que `recu` moins l'antidatage permis (§ 4.4).
    pub fait: Instant,
    /// Le pas où la partie qui fait foi l'a reçu.
    pub recu: NumPas,
    /// Le pas de la dernière vue ou projection que l'appareil avait reçue.
    pub vu: NumPas,
    /// L'identité du geste, stable à travers les renvois et les rejeux.
    pub id: IdGeste,
}

/// Un instant à l'intérieur d'un pas, pour départager une course serrée.
pub struct Instant { pub pas: NumPas, pub fraction: u16 }
```

- `place` est toujours le pas en cours. `fait` peut être plus tôt, si le geste a été fait avant la fenêtre (§ 4.4). `vu` est toujours au plus `fait`.
- `vu` sert à refuser un geste périmé, sans numéro de phase écrit à la main : le jeu garde le pas où sa phase a commencé, et compare.
- `id` sert à nommer ce qu'un geste crée (un tir, un trait), pour que la prédiction et la partie qui fait foi le nomment pareil (§ 4.6).

```rust
pub enum DeEcran<A> {
    /// Un bouton ou un raccourci que la vue de l'écran affichait.
    Action { action: A, quand: Quand },
    /// Une touche brute, si la vue l'a demandée. Répétition écartée, sauf demande.
    Touche { touche: Touche, quand: Quand },
    /// Un pointeur brut : appui, déplacement, relâché, annulation.
    Pointeur { evenement: Pointeur, quand: Quand },
    /// Le résultat d'un composant : texte édité, élément déposé, fichiers reçus.
    Composant { evenement: Composant, quand: Quand },
}

pub enum Presence {
    Arrivee { appareil: Appareil, role: Role, capacites: Capacites, prenom_app: Option<Prenom> },
    Depart { appareil: Appareil, cause: CauseDepart },
    Retour { appareil: Appareil, capacites: Capacites },
    Capacites { appareil: Appareil, capacites: Capacites },
}

pub enum Role { Ecran, Telephone, Manette }
pub enum CauseDepart { Silence, Fermeture, Voulu }
```

Un onglet jumeau qui prend la place n'est pas un départ (SAL-154). Le jeu ne le voit pas. Une page fermée pour laisser place au pad natif n'en est pas un non plus (SQ-89). Le jeu ne voit ni lien, ni session, ni jeton (SAL-08, SAL-15).

### 1.5 Les états tenus

```rust
pub trait Tenu: Message + PartialEq {
    /// Plusieurs états tenus par appareil, un par clé (un brouillon par consigne).
    type Cle: Message + Ord;
    fn cle(&self) -> Self::Cle;
    /// La valeur quand on relâche. `None` : jamais relâché seul (un brouillon).
    fn neutre() -> Option<Self>;
    /// Un relâchement passe toujours, quel que soit son âge.
    fn est_neutre(&self) -> Neutre;
}

impl Tenu for () { /* aucun état tenu */ }

/// Les règles d'un état tenu, déclarées une fois.
pub struct ReglesTenu {
    /// Relâché si l'appareil se tait plus longtemps. `SansObjet` pour un brouillon.
    pub fraicheur: Applicable<Duree>,
    /// Un état fait plus tôt que cela ne compte pas (un neutre passe toujours).
    pub age_max: Applicable<Duree>,
    /// Au plus tant de changements par seconde et par clé, au-delà le dernier gagne.
    pub debit_max: u16,
}
```

Le commun livre un `Changement` à chaque changement de la **valeur effective**. Une répétition de la même valeur ne coûte rien au jeu. La page n'écrit aucune répétition : le commun reprend la valeur tenue au tiers de la fraîcheur (150 ms pour 450 ms), et cesse quand la page se cache (SQ-09). Sur l'appareil, une coupure du lien ne lâche rien (REM-10). Seul le départ du premier plan relâche tout (REM-11). Sur la partie qui fait foi, le silence relâche (SQ-01).

Les types communs d'un état tenu :

```rust
/// Un axe fini dans [-1, 1], sans sous-normal. Décodé ainsi ou refusé.
pub struct Axe(/* f32 */);
/// Deux axes de norme au plus 1, zone morte de l'appareil appliquée.
pub struct Axe2 { /* x, y */ }
/// Un bouton : tenu ou non, et le nombre d'appuis depuis l'arrivée de l'appareil.
pub struct Bouton { pub etat: Appui, pub appuis: Cumul }
pub enum Appui { Tenu, Relache }
/// Un compteur qui ne fait que monter (appuis, visée cumulée). Absolu, jamais un écart.
pub struct Cumul(/* u32, en boucle */);
impl Cumul { pub fn depuis(self, avant: Cumul) -> u32; }
/// Une manette standard : deux sticks, deux gâchettes, seize boutons.
pub struct Manette { /* ... */ }
```

`Bouton.appuis` règle deux leçons d'un coup. Un appui plus court qu'un envoi laisse une trace (REM-16, CAT-16). Deux appuis dans le même envoi comptent deux (REM-17). Le compteur est absolu : un paquet perdu ne décale rien (REM-13).

### 1.6 Le temps

```rust
pub struct NumPas(/* u64 */);
pub struct Duree(/* u64, en pas */);
pub struct Periode { /* num / den secondes */ }

impl Periode { pub const fn hz(n: u32) -> Periode; }
impl Duree {
    /// Arrondi au-dessus : 450 ms à 60 Hz font 27 pas.
    pub const fn ms(ms: u64, p: Periode) -> Duree;
    pub const fn pas(n: u64) -> Duree;
    pub const fn en_ms(self, p: Periode) -> u64;
}
impl NumPas {
    pub const fn plus(self, d: Duree) -> NumPas;     // sature
    pub const fn depuis(self, avant: NumPas) -> Duree; // 0 si avant est plus tard
}
```

Pas de flottant, pas d'heure, pas de somme de durées en virgule. Une échéance est un `NumPas` dans l'état. Le jeu ne programme aucun minuteur.

### 1.7 Le hasard

```rust
/// 256 bits, neuve à chaque partie, sauf rejeu voulu.
pub struct Graine(/* [u8; 32] */);

/// Le générateur du jeu, dans son état. Déterministe, sérialisé avec lui.
#[derive(Etat)]
pub struct Hasard { /* xoshiro256** */ }

impl Hasard {
    pub fn depuis(g: Graine) -> Hasard;
    pub fn entier(&mut self) -> u64;
    /// Sans biais (méthode de Lemire). Refuse 0.
    pub fn borne(&mut self, n: core::num::NonZeroU64) -> u64;
    /// Fisher-Yates.
    pub fn melanger<T>(&mut self, v: &mut [T]);
    pub fn choisir<'v, T>(&mut self, v: &'v [T]) -> Option<&'v T>;
    pub fn une_sur(&mut self, n: core::num::NonZeroU32) -> Tirage;
    /// Un générateur indépendant, pour une sous-partie.
    pub fn deriver(&mut self) -> Hasard;
}
```

Le rendu a son propre générateur, tiré de la clé de chaque élément dessiné, hors de l'état.

### 1.8 Les appareils, les regards et les secrets

```rust
/// Un appareil, créé par le commun seul. Opaque : son ordre ne veut rien dire.
pub struct Appareil(/* u32 */);

/// Qui regarde : l'écran, un téléphone. Construit par le commun seul.
pub struct Regard { /* privé */ }
impl Regard {
    pub fn role(&self) -> Role;
    pub fn appareil(&self) -> Option<Appareil>;
}

/// Le jeton qui ouvre un voile. Seule l'étape et l'épreuve le reçoivent.
pub struct Autorite { /* privé, ni Clone ni Copy */ }

/// Une valeur que tout le monde ne voit pas.
#[derive(Etat)]
pub struct Voile<T: Etat> { /* qui: Qui, valeur: Option<T> */ }

pub enum Qui { Personne, Appareil(Appareil), Appareils(BTreeSet<Appareil>), Ecran, Tous }

pub struct Absent;

impl<T: Etat> Voile<T> {
    pub fn cacher(qui: Qui, valeur: T) -> Voile<T>;
    /// Pour l'étape. Sur un appareil qui prédit, rend souvent `Absent`.
    pub fn lire(&self, a: &Autorite) -> Result<&T, Absent>;
    pub fn lire_mut(&mut self, a: &Autorite) -> Result<&mut T, Absent>;
    /// Pour une vue ou un bot : seulement si ce regard a le droit.
    pub fn voir(&self, r: &Regard) -> Option<&T>;
    /// Changer qui voit, par exemple à la révélation.
    pub fn montrer(&mut self, qui: Qui);
    pub fn qui(&self) -> &Qui;
}
```

La forme d'un voile est publique : qu'il existe, et qui peut le voir. Sa valeur ne l'est pas. Pour cacher combien il y a de réponses, on voile la liste entière, pas chaque réponse. La section 4 dit le reste.

### 1.9 Les vues

```rust
/// Une vue de téléphone, envoyée à la page. Dérivée pour la fusion et la galerie.
pub trait Vue: Message { type Variante: Message + Ord + Copy; fn variante(&self) -> Self::Variante; }

/// Une valeur qui ne s'affiche qu'une fois définitive.
#[derive(Message)]
pub struct Definitif<T: Message> { /* cle: Cle, valeur: Option<T> */ }
impl<T: Message> Definitif<T> {
    pub fn de(cle: Cle, valeur: T) -> Definitif<T>;
    /// Sur la page : `None` tant que ce n'est pas sûr.
    pub fn valeur(&self) -> Option<&T>;
}

/// Une clé stable, calculée depuis une valeur du jeu (pas depuis une position).
pub struct Cle(/* u64 */);
impl Cle { pub fn de(v: &impl Message) -> Cle; }
```

La scène de l'écran est le langage de dessin du commun. Seule sa part temporelle est décrite ici :

```rust
impl<A: Message> Scene<A> {
    pub fn noeud(&mut self, cle: Cle, n: Noeud) -> &mut Noeud;
    /// Ce sous-arbre ne vient que de l'instance définitive.
    pub fn definitif(&mut self, cle: Cle, f: impl FnOnce(&mut Scene<A>));
    /// Un compte à rebours que le commun anime seul, depuis l'échéance.
    pub fn compte_a_rebours(&mut self, cle: Cle, c: CompteARebours) -> &mut Noeud;
    pub fn bouton(&mut self, cle: Cle, libelle: Texte, action: A) -> &mut Noeud;
    pub fn raccourci(&mut self, touche: Touche, action: A);
    pub fn clavier_brut(&mut self, c: ClavierBrut);
}

impl Noeud {
    /// Glisser entre deux pas, si la même clé existe aux deux.
    pub fn glisse(&mut self) -> &mut Noeud;
    /// Ne pas glisser à ce pas : un saut voulu (téléportation, réapparition).
    pub fn rupture(&mut self) -> &mut Noeud;
    /// Lisser une correction de prédiction sur cette durée.
    pub fn lisse(&mut self, d: Duree) -> &mut Noeud;
    /// Arrondir la position affichée au pixel entier du monde.
    pub fn pixel_entier(&mut self) -> &mut Noeud;
    /// Une animation qui part d'un instant de la partie, jamais du réaffichage.
    pub fn anime(&mut self, depuis: Instant, a: Animation) -> &mut Noeud;
}

pub enum CompteARebours {
    Court { echeance: NumPas, arrondi: Arrondi },
    Fige { restant: Duree, arrondi: Arrondi },
}
```

### 1.10 Les effets et les demandes

```rust
pub struct Effet {
    /// Une identité sémantique : la même chose à rejouer porte la même identité.
    pub id: IdEffet,
    /// L'instant où l'effet doit se produire, sur l'horloge de la partie.
    pub quand: Instant,
    pub pour: Destinataire,
    pub quoi: Sensation,
    /// Joué dès la tête, ou seulement une fois le pas définitif.
    pub tenue: Tenue,
    /// Au-delà de `quand` plus cette durée, l'effet n'est plus joué.
    pub peremption: Duree,
}

impl IdEffet { pub fn de(v: &impl Message) -> IdEffet; }
pub enum Destinataire { Ecran, Appareil(Appareil), Telephones, Tous }
pub enum Tenue { Provisoire, Definitive }
pub enum Sensation { Son(Son), Vibration(Vibration) }

pub enum Demande {
    Ecrire { flux: Flux, octets: Vec<u8> },
    Telecharger { nom: NomFichier, octets: Partage<[u8]>, sorte: SorteFichier },
    Image { transformation: Transformation, groupe: Groupe },
    RenduHorsEcran { rendu: Rendu, groupe: Groupe },
    Annuler(IdDemande),
    Oublier(Appareil),
    Retirer(Appareil),
    Fermer,
    Ouvrir,
    NouvellePartie,
    Marquer(Texte),
}

pub enum Groupe { Independante, Remplace(Cle) }

pub struct Reponse { pub id: IdDemande, pub resultat: Resultat }
```

`Tenue` et `peremption` n'ont pas de valeur par défaut. Chaque effet dit s'il peut se tromper.

### 1.11 Ce qui vient de l'extérieur

Chaque type qui traverse une frontière se valide à son décodage. Le jeu ne reçoit jamais une valeur invalide.

| Type | Garantie au décodage |
|---|---|
| `Axe`, `Axe2` | fini, borné, norme au plus 1, sous-normal ramené à 0 |
| `Fini32`, `Fini64` | ni NaN ni infini |
| `Borne<T, MIN, MAX>` | dans la plage, sinon refusé |
| `Texte<N>` | nettoyé (contrôles, ignorables, NFC), coupé entre graphèmes, au plus N |
| `Prenom` | `Texte` plus la règle d'encre de la police déclarée et les noms réservés du jeu |
| `Cumul` | monotone par appareil, vérifié par le commun |
| tout message | le cadre est consommé en entier, sans octet restant (SAL-118) |

Un décodage qui échoue n'atteint pas le jeu. Il coûte à son lien, pas à la partie (SAL-117).

### 1.12 La déclaration

```rust
pub struct Declaration<J: Jeu> {
    pub nom: &'static str,
    pub version_etat: u32,
    pub places: Places,                       // joueurs min et max, appareils max
    pub temps: Temps,
    pub fenetre: Applicable<Duree>,
    pub antidatage_max: Duree,
    pub pause: Pause,
    pub ecran: FormeEcran,
    pub telephones: Applicable<Telephones<J>>,
    pub manettes: Applicable<ReglesTenu>,
    pub hasard: Applicable<()>,
    pub fin: Applicable<()>,
    pub limites: Limites,
    pub touches_du_commun: TouchesDuCommun,
    pub mecaniques: &'static [Mecanique],
}

pub enum Applicable<T> { Oui(T), SansObjet(Raison) }
pub struct Raison(&'static str);              // vide : refusé par l'épreuve

pub enum Temps {
    PasFixes { periode: Periode, rattrapage_max: u8 },
    ALaDemande { periode: Periode, rattrapage_max: u8 },
    Aucun(Raison),
}

pub enum Pause { DuJeu, DuCommun, Aucune(Raison) }

pub struct Telephones<J: Jeu> {
    pub tenu: ReglesTenu,
    pub prediction: Applicable<()>,
    pub capacites_utiles: &'static [Capacite],
}

pub struct Limites {
    pub geste_octets: u32,
    pub tenu_octets: u32,
    pub file_evenements: u16,
    pub vue_octets: u32,
    pub effets_par_pas: u16,
    pub memoire_octets: u64,
}
```

`Declaration` n'implémente pas `Default`. Chaque champ s'écrit. Une exemption est un `SansObjet` avec sa raison : omettre un champ ne compile pas (HIS-04, OUT-03, OUT-04). Les limites sont celles de l'autorité, annoncées à l'accueil de chaque appareil (SAL-121).

### 1.13 Les dérivations

| Dérive | Ce qu'elle écrit |
|---|---|
| `#[derive(Etat)]` | la forme canonique (sauvegarde, comparaison, conversion), la projection par regard, le schéma, l'interdiction de la mutabilité intérieure, l'énumération des champs pour les mutations de l'épreuve |
| `#[derive(Message)]` | le format `postcard` validé au décodage, le schéma |
| `#[derive(Vue)]` | `Message`, la fusion des `Definitif` par clé, l'énumération `Variante` pour la galerie |

Aucune n'est écrite par le jeu à la main. Les fonctions canoniques ne sont pas appelables depuis le code du jeu : leur trait est scellé et prend un jeton que seul `jeu::module!` détient. Une vue ne peut donc pas sérialiser un voile pour en lire les octets.

---

## 2. Le contrat binaire du module

### 2.1 La cible

- `wasm32-unknown-unknown`, Rust 1.99, édition 2024.
- `panic = "abort"` : une panique est un `unreachable`, donc une faute isolée (§ 3.4).
- Lien avec `--stack-first` : la pile est en bas de la mémoire, un débordement de pile tombe hors de la mémoire et s'arrête.
- Pas de fils, pas d'atomiques partagés, pas de SIMD relâché. Pas de SIMD flottant au départ (question ouverte, § 9).
- **Aucun import.** La bibliothèque standard compile sans en demander sur cette cible. Ce qu'un jeu voudrait journaliser passe par ses sorties, en mode épreuve.

### 2.2 La mémoire

La mémoire linéaire unique se découpe en zones, que `jeu_zones` décrit une fois pour toutes.

| Zone | Contenu | Fait partie de l'état |
|---|---|---|
| Pile | la pile de Rust | non : rien ne la relit entre deux appels |
| Statiques | `.data`, `.bss`, l'état de l'allocateur | oui |
| Échange | entrées et sorties des appels, taille fixe déclarée | non : remise à zéro après chaque appel |
| Brouillon | les allocations d'un appel en lecture, taille fixe déclarée | non : remise à zéro après chaque appel |
| Tas | tout ce que l'état alloue | oui |

L'allocateur est celui de la crate `jeu`, installé par la macro. Il a deux modes. En écriture (`jeu_entree`, `jeu_creer`, `jeu_charger`), il sert le tas. En lecture (`jeu_vue`, `jeu_bot`…), il sert le brouillon, en pile, remis à zéro à la fin de l'appel. Une vue qui alloue ne touche donc pas à l'état.

**L'état, c'est les statiques et le tas, et la taille de la mémoire.** Le moteur copie, restaure, hache et compare exactement cela. Deux exécutions du même module sur les mêmes entrées ont ces zones identiques, octet pour octet.

### 2.3 Les exports

Chaque fonction reçoit la longueur de son entrée, écrite par le moteur au début de la zone d'échange. Elle rend un `i64` : position (32 bits hauts) et longueur (32 bits bas) de sa sortie dans la zone d'échange. La sortie est un `postcard` de `Sortie<T>`.

| Export | Signature | Entrée → sortie | Sorte |
|---|---|---|---|
| `memory` | mémoire | | |
| `jeu_abi` | `() -> i32` | version majeure de l'ABI | |
| `jeu_init` | `() -> ()` | prépare l'allocateur, l'échange, le brouillon | une fois |
| `jeu_zones` | `() -> i32` | position d'une table fixe des zones | une fois |
| `jeu_declarer` | `(i32) -> i64` | `()` → `Declaration`, schéma, empreinte des types | lecture |
| `jeu_creer` | `(i32) -> i64` | `Creation` → `Ok` ou `Refus` | écriture |
| `jeu_valider` | `(i32) -> i64` | octets d'un appareil → `Valide` ou `Malforme` | lecture |
| `jeu_entree` | `(i32) -> i64` | `Entree` → `IssueOctets` (sorte, refus, effets, demandes, besoin de pas) | écriture |
| `jeu_vue` | `(i32) -> i64` | `Regard` → `Scene` ou `VueTelephone` | lecture |
| `jeu_projeter` | `(i32) -> i64` | `Regard` et morceau → projection | lecture |
| `jeu_charger` | `(i32) -> i64` | projection → l'état devient elle | écriture |
| `jeu_phase` | `(i32) -> i64` | `()` → `Phase` | lecture |
| `jeu_attente` | `(i32) -> i64` | `()` → `Attente` | lecture |
| `jeu_garder` | `(i32) -> i64` | `()` → `Garde` | lecture |
| `jeu_canonique` | `(i32) -> i64` | morceau → forme canonique, secrets compris | lecture |
| `jeu_depuis_canonique` | `(i32) -> i64` | forme canonique et sa version → `Ok` ou `Refus` | écriture |
| `jeu_invariants` | `(i32) -> i64` | `()` → `Rapport` | lecture, épreuve |
| `jeu_transition` | `(i32) -> i64` | état d'avant canonique et entrée → `Rapport` | lecture, épreuve |
| `jeu_bot` | `(i32) -> i64` | `DemandeBot` → gestes du bot | lecture, épreuve |
| `jeu_galerie` | `(i32) -> i64` | `Phase` → pires cas canoniques | lecture, épreuve |
| `jeu_memoire` | `(i32) -> i64` | `()` → octets vivants du tas | lecture |

Une sortie plus grande que la zone d'échange se lit en morceaux : l'appel est répété avec un numéro de morceau. C'est rare (forme canonique d'un gros état) et cela reste déterministe.

`jeu_valider` existe pour que rien de malformé n'entre au journal. Le moteur l'appelle avant de placer un geste.

### 2.4 Le format

- `postcard` 1.x, qui fige son format. Entiers en varint, flottants en petit-boutiste, énumérations par indice.
- Un cadre se décode en entier. Un octet restant est un `Malforme`.
- Les types sont les mêmes types Rust des deux côtés : la crate `jeu` pour les types communs, la crate de types du jeu pour les siens. Le Component Model viendra plus tard et remplacera ce format sans changer les types.
- Le schéma de chaque type est décrit par sa dérive. Son empreinte (FNV-1a 64, calculée à la compilation) part dans la déclaration.

### 2.5 Les versions

| Version | Calculée par | Sert à |
|---|---|---|
| ABI | constante de la crate `jeu` | refuser un module que le moteur ne sait pas appeler |
| Module | le moteur, BLAKE3 des octets du module | l'identité de la partie, des sauvegardes, des journaux |
| Sources | l'outil, BLAKE3 des sources empaquetées | relier un module à ses sources (OUT-37) |
| Types | la dérive, empreinte des schémas | refuser une page restée en cache (SAL-41) |
| État | `version_etat` déclarée par le jeu | choisir la conversion d'une sauvegarde |

Une sauvegarde de la même empreinte de module se restaure par sa mémoire. Une sauvegarde d'une autre empreinte passe par la forme canonique : même `version_etat`, elle se recharge ; `version_etat` précédente, elle passe par `convertir` ; sinon, refus net. Une forme canonique abîmée donne un refus, jamais un état à moitié lu (HIS-51).

### 2.6 Ce que le moteur réécrit au chargement

Le moteur lit le module et le réécrit avant de l'instancier. Il refuse ce qu'il ne sait pas garantir.

1. **Vérifier.** Aucun import. Exactement les exports du § 2.3. Pas de `start`. Pas de fils, d'atomiques, de SIMD relâché. Pas de `table.set` ni de `table.grow`. Une mémoire, de taille maximale au plus `memoire_octets`.
2. **Compter.** Un compteur d'instructions dans un global ajouté : décrémenté à l'entrée de chaque fonction et en tête de chaque boucle, du coût du bloc. À zéro, `unreachable`. Le moteur le remplit avant chaque appel, selon la sorte d'appel.
3. **Canoniser les NaN.** Après chaque opération flottante qui peut produire un NaN, un NaN devient le NaN canonique. C'est la seule source de non-déterminisme de WebAssembly sans fils ni SIMD relâché.
4. **Surveiller la croissance.** Chaque `memory.grow` passe par une fonction du moteur qui refuse au-delà de la limite. Le refus rend `-1`, l'allocateur échoue, le jeu s'arrête : une faute.
5. **Garder les globals.** Le moteur vérifie entre deux appels que les globals du module (le pointeur de pile) ont retrouvé leur valeur.

Le module réécrit est celui qui passe l'épreuve et celui qu'on joue. La réécriture est une fonction pure des octets d'entrée : son résultat a sa propre empreinte, comparée partout.

---

## 3. Le moteur : appeler, couper, reprendre, isoler, comparer

### 3.1 L'horloge de la partie

Le moteur tient une seule horloge, celle de la partie. Elle avance avec l'horloge monotone de l'ordinateur de la soirée, sauf :

- quand le commun met la partie en pause (`Pause::DuCommun`) ;
- quand la machine prend plus de retard que `rattrapage_max` pas : le reste est jeté.

Le numéro de pas est `⌊temps de partie / période⌋`. Il ne saute jamais : chaque appel de `Pas` a le numéro précédent plus un. Un gel de dix minutes ne traverse donc aucune phase : à la reprise, la partie continue où elle était, et s'allonge du temps perdu (PUN-34, CAT-15). Les appareils voient l'horloge glisser par leurs échanges réguliers (§ 4.4).

L'horloge monotone peut ne pas compter le sommeil (SAL-51). Le moteur la recoupe avec l'heure murale à chaque image. Un écart qui ne s'explique pas est un gel, traité comme tel, et noté au journal.

### 3.2 La boucle de la partie qui fait foi

Le moteur tourne dans un worker de l'ordinateur de la soirée. La télé dessine dans la page. Chaque image :

```
t ← horloge de la partie
cible ← ⌊t / période⌋, au plus tete.pas + rattrapage_max (le reste est jeté)

pour chaque message arrivé :
    décoder l'enveloppe du commun ; dédoublonner (appareil, session, numéro)
    jeu_valider sur une instance quelconque → Malforme : compté contre le lien, fin
    placer le geste :
        fait  ← instant estimé, borné par l'antidatage permis
        place ← max(fait, définitive.pas + 1)
    si place.pas ≤ tete.pas : retour ← min(retour, place.pas)

tant que définitive.pas < cible − fenêtre :
    avancer la définitive d'un pas (§ 3.3) avec les entrées placées à ce pas
    écrire ce pas au journal
    exécuter ses demandes ; jouer ses effets « Definitive » ; vues définitives

si retour existe :
    tête ← copie de la définitive ; rejouer de définitive.pas + 1 jusqu'à tete.pas
avancer la tête jusqu'à cible
jouer les effets « Provisoire » nouveaux ; annuler ceux que la tête ne réémet plus

vue de l'écran : tête, avec les sous-arbres « definitif » de la définitive
vues ou projections des appareils : depuis la définitive (§ 4.3)
```

Sans fenêtre, la tête est la définitive : une seule instance, pas de copie, pas de rejeu.

### 3.3 Appeler l'étape

Un pas `s` se joue ainsi, sur une instance :

1. Les entrées placées à `s`, dans l'ordre des instants `fait`. À instant égal, l'ordre est tiré d'un hachage de la graine d'ordre, du pas et de l'identité *interne* de l'appareil, jamais de l'`Appareil` exposé (CAT-27, § 6.4).
2. Puis `Pas(s)`.

Chaque entrée est un appel de `jeu_entree`. Le moteur lit l'issue :

| Issue | Ce que fait le moteur |
|---|---|
| Acceptée | garde l'état, prend ses effets et ses demandes |
| Refusée | rien ne change ; le refus part au seul appareil du geste, sans changer sa vue |
| Ignorée | rien ne change |
| En attente | sur la partie qui fait foi : faute du jeu. Sur un appareil qui prédit : ce geste attend la confirmation (§ 4.5) |
| Faute | § 3.4 |

Après chaque entrée acceptée, `besoin_de_pas` dit si le jeu veut des pas. Un jeu `ALaDemande` n'en reçoit que tant qu'il en veut, avec le même plafond de rattrapage au réveil d'un onglet suspendu.

### 3.4 Couper un pas trop long, isoler une entrée fautive

Une faute, c'est : compteur d'instructions épuisé, `unreachable` (panique), accès hors mémoire, débordement de pile, croissance refusée, sortie trop grande, sortie indécodable, effets au-delà de la limite.

Après une faute dans un appel en écriture, la mémoire est peut-être à moitié modifiée. Le moteur ne la croit plus. Il fait toujours la même chose :

1. **Restaurer.** L'instance fautive reprend la mémoire de la définitive, ou du dernier point de reprise si c'est la définitive qui a fauté.
2. **Isoler.** L'entrée fautive est écrite au journal comme `Ecartee { cause }`. Les rejeux la sautent, à l'identique. Un geste écarté répond `Refus::Interne` à son appareil.
3. **Rejouer** le journal jusqu'au pas courant, sans elle.
4. **Garder la preuve.** Le moteur range la graine, le point de reprise, les entrées jusqu'à la faute et l'entrée isolée : de quoi rejouer l'échec seul (HIS-06, HIS-07).

Une faute dans `Pas(s)` écarte ce pas : le jeu reçoit ensuite `Pas(s + 1)`, et voit le trou. Si trois pas de suite fautent, la partie ne peut plus avancer. La télé dit « Le jeu est en panne », propose de reprendre au dernier point sain sans les entrées qui ont suivi, ou une nouvelle partie. La page ne meurt pas, le lien des appareils reste, et le journal est gardé. On ne promet pas mieux : un jeu dont le temps seul plante doit être pris par l'épreuve avant la soirée.

Une faute dans un appel en lecture (une vue, un bot) ne touche pas l'état. Le moteur garde la dernière vue valide, signale la faute, et vérifie l'empreinte de l'état. Si elle a changé, il restaure comme ci-dessus.

Le budget se compte en instructions, pas en temps. Une même entrée coûte la même chose partout, dans le navigateur et dans les tests : une faute de budget se rejoue. Le moteur étalonne une fois le prix d'une instruction sur la machine, et l'épreuve vérifie que `(fenêtre + 2) × coût d'un pas au 99ᵉ centile` tient dans une image.

### 3.5 Reprendre après un plantage

Un point de reprise est une copie compressée des zones d'état de la définitive, avec l'état du commun : présences, compteurs des liens, identités d'effets déjà joués, demandes ouvertes, graine. Il est pris toutes les quelques secondes et à chaque nouvelle partie. Le journal garde chaque pas définitif depuis.

Les deux sont gardés hors de l'onglet, chiffrés : dans le stockage du navigateur, et par le relais. À la recharge :

1. charger le module de la même empreinte ;
2. restaurer le point de reprise ;
3. rejouer le journal, et comparer l'empreinte d'état du dernier point du journal ;
4. reprendre la boucle. Les appareils renvoient leurs événements non accusés ; le dédoublonnage par appareil et par session les compte une fois, même s'ils avaient atteint l'exécution d'avant (SAL-57).

Un rejeu qui ne retrouve pas l'empreinte est une panne du commun, pas du jeu. Le moteur le dit, et garde les deux.

### 3.6 Comparer deux exécutions

| Comparaison | Ce qu'on hache | Où |
|---|---|---|
| Deux exécutions du module | zones d'état, à chaque pas | épreuve |
| Module et copie native | forme canonique, à chaque pas | épreuve |
| Avec et sans prédiction | journal définitif, entrées et issues | épreuve |
| Identités permutées | vues de chaque appareil, permutées | épreuve |
| Rejeu après reprise | zones d'état au point de contrôle | toujours |

À la première divergence, le rapport dit le pas, l'entrée, l'adresse, et le chemin du champ qui diffère, retrouvé par la forme canonique des deux états.

Pendant l'épreuve, le moteur hache aussi l'état avant et après chaque appel en lecture et chaque refus. Une différence est une faute du jeu : un `Partage` gardé, une statique écrite, un refus qui a laissé une trace.

---

## 4. Secrets, projection et prédiction

### 4.1 Trois instances possibles

| Instance | Où | Sa mémoire | Elle sert à |
|---|---|---|---|
| Définitive | ordinateur de la soirée | l'état qui fait foi, jusqu'au pas définitif | journal, demandes, projections, `Definitif` |
| Tête | ordinateur de la soirée, si fenêtre | une copie de la définitive, plus la fenêtre | l'image de la télé, les effets provisoires |
| Prédiction | appareil qui prédit | une projection reçue, plus ses propres gestes | l'image de cet appareil |

Une prédiction ne décide jamais : la définitive ne reçoit que des gestes.

### 4.2 Le secret dans les types

Un secret est un `Voile<T>`. Sur l'ordinateur de la soirée, il contient sa valeur. La projection pour un regard vide chaque voile que ce regard ne voit pas. Un appareil ne peut pas montrer un secret qu'il n'a pas reçu.

Trois portes, et pas une de plus :

1. **L'étape** reçoit `Autorite`, et lit avec `lire`. C'est la seule qui peut copier un secret dans un champ public. Elle le fait en clair dans le code, et c'est ce qu'un relecteur cherche.
2. **Une vue ou un bot** reçoit un `Regard`, et lit avec `voir`. Le regard de la télé est `Ecran` : la télé ne voit que `Ecran` et `Tous`. La télé passe donc elle aussi par sa projection, sans copie, alors qu'elle tourne sur la machine qui détient tout (CAT-46).
3. **`montrer`** change qui voit. Une révélation est un changement d'état, fait par l'étape.

Ce que les types n'empêchent pas : l'étape peut écrire `self.public = voile.lire(a)?.clone()`. Le commun le cherche (§ 6.3) : il connaît les octets de chaque valeur voilée, et les cherche dans chaque projection et chaque vue où ils devraient manquer.

### 4.3 Ce que reçoit chaque appareil

| L'appareil | Il reçoit | Il calcule |
|---|---|---|
| ne prédit pas (Sidequest, Punchline) | sa vue, calculée sur l'ordinateur de la soirée depuis la définitive, avec son regard | rien |
| prédit (un FPS) | sa projection de la définitive, et le module | sa vue, depuis sa prédiction |

Les deux chemins donnent la même vue : une vue ne lit les voiles qu'à travers son regard, donc `vue(état, regard)` égale `vue(projection(état, regard), regard)`. L'épreuve le vérifie à chaque pas. Le premier chemin évite d'envoyer le module à 71 téléphones qui n'en ont pas besoin. Il évite aussi d'envoyer la salle entière de Sidequest à des téléphones qui ne la montrent pas.

Une vue ou une projection est un état : la plus récente remplace l'autre, renvoyée à l'arrivée et à chaque reconnexion (SAL-28, SAL-112). Une vue inchangée ne repart pas. Pas de delta (SAL-113). Une vue est toujours celle d'un pas, jamais d'un instant d'arrivée (SQ-86).

### 4.4 L'heure d'un geste

L'appareil estime l'horloge de la partie par le plus court aller-retour récent (SAL-128, SQ-10). Il date chaque geste : `fait` en pas et fraction. La partie qui fait foi borne cette date :

```
fait ← max(fait annoncé, recu − aller-retour mesuré / 2 − antidatage_max)
place ← max(fait, définitive.pas + 1)
```

- Un appareil ne peut pas antidater au-delà de sa demi-latence mesurée plus une marge déclarée (SAL-129). La latence se mesure par écho depuis l'ordinateur de la soirée : un appareil peut la grossir, pas la réduire.
- Dans la fenêtre, un geste tombe à son moment : un vote fait pendant un duel tombe sur ce duel.
- Au-delà de la fenêtre, il tombe au premier pas encore provisoire, avec son vrai `fait` et son `vu`. Le jeu peut le refuser sur `vu` (CAT-44, SAL-115).
- Une heure d'appareil ne recule jamais, même après une reconnexion. Elle s'arrête plutôt que de reculer.

La fraîcheur d'un état tenu se compte sur `fait`. Si un renvoi en retard, mais dans la fenêtre, montre que l'appareil ne s'était pas tu, le relâchement par silence disparaît au rejeu. Une fenêtre plus large que les blocages du repli WebSocket efface les relâchements d'un instant que SQ-91 craignait.

### 4.5 Prédire, c'est exécuter la même étape

Sur un appareil qui prédit, à chaque projection reçue au pas `d` :

```
charger la projection (jeu_charger)
pour s de d + 1 à maintenant estimé :
    appliquer ses propres gestes prévisibles placés à s, non encore inclus
    appliquer son propre état tenu à s
    Pas(s)
```

- Seuls les gestes de cet appareil sont ajoutés. Les autres joueurs avancent selon ce que la projection contient : leurs états tenus visibles continuent. C'est une extrapolation par l'étape, pas une invention de leurs gestes.
- `previsible` dit, geste par geste, ce qui s'applique avant confirmation. Par défaut, rien.
- Un `Voile` vide rend `Absent`. Le jeu écrit `attendre(absent)`. Avant acceptation, rien n'a changé : le geste reste « en attente ». Après acceptation, le moteur reconstruit la prédiction depuis la projection et marque le geste en attente. Sur la partie qui fait foi, `Absent` n'arrive jamais : un voile y a toujours sa valeur, sinon c'est une faute.
- La projection dit, pour cet appareil, le dernier événement et le dernier état tenu qu'elle inclut. La prédiction ne rejoue que ce qui suit.
- Si la prédiction au pas `d` égale la projection reçue (même empreinte canonique), rien ne bouge à l'écran.
- Une correction se lisse, nœud par nœud, sur la durée que la vue déclare.
- La prédiction peut toujours être coupée : un appareil lent, suspendu, ou dont l'instance a fauté, montre la projection seule. La reprise est la même.

### 4.6 Nommer ce qu'un geste crée

Un tir prédit sur l'appareil crée une balle. La partie qui fait foi crée la même balle. Si chacun la numérote avec son propre compteur, la vue en montre deux, puis une disparaît. Règle : ce qu'un geste crée se nomme depuis `quand.id`, jamais depuis un compteur de l'état. La clé de dessin et l'identité d'effet en découlent. L'épreuve compte les nœuds qui apparaissent puis disparaissent au moment d'une confirmation.

### 4.7 La fenêtre, selon le genre

| Genre | Fenêtre | Pourquoi |
|---|---|---|
| Tour par tour (Punchline) | aucune | `vu` suffit ; une échéance compte ce qui est arrivé avant elle |
| Éditeur sur une seule machine (Tier list) | aucune | tout est local |
| Foule en temps réel (Sidequest) | 6 pas (100 ms), à mesurer | les positions à l'échéance suivent les vrais gestes |
| FPS sur téléphones | 12 pas (200 ms), à mesurer | couvre l'aller-retour et la gigue du Wi-Fi |

Le coût d'une fenêtre : une copie de mémoire et `fenêtre` pas rejoués par image où un geste arrive en retard. Un résultat attend la fenêtre pour s'annoncer.

---

## 5. Vues, effets et demandes

### 5.1 Fusionner la tête et le définitif

Le moteur calcule chaque vue deux fois : sur la tête et sur la définitive. Puis il fusionne :

- tout ce qui n'est pas marqué vient de la tête ;
- chaque `Definitif<T>` et chaque sous-arbre `definitif` vient de la définitive, **par clé**. Une clé que la définitive n'a pas encore rend `None`.

La fusion se fait par clé et non par position. La tête peut être déjà aux Résultats quand la définitive est encore à la Question : le panneau des résultats y reste vide, au lieu d'être mal apparié. Sans fenêtre, un seul calcul.

La télé montre donc les personnages au plus tôt, et les « +1 » seulement quand ils sont sûrs (CAT-41).

### 5.2 Glisser entre deux pas

La télé dessine à un instant légèrement passé, entre deux scènes de la tête. Un nœud glisse si la même clé existe aux deux pas et qu'aucun ne porte `rupture`. Le reste s'affiche tel quel.

- La télé et le moteur ne sont pas dans le même fil. L'instant de dessin suit l'horloge des arrivées de scènes, avec une marge au-delà d'un pas (SQ-56, SQ-58). La marge se règle sur la part d'images figées mesurée, sur plusieurs passes (SQ-57).
- Une clé revenue avec un autre contenu ne glisse pas : la clé se calcule depuis l'identité de l'objet, et le jeu marque `rupture` quand l'objet saute (TL-23).
- `pixel_entier` arrondit la position glissée au pixel du monde : le pixel art reste sur sa grille.
- Le commun compte ses présentations réussies, côté GPU, et non les tours de `requestAnimationFrame` (OUT-33).

### 5.3 Animer sans rejouer

Une animation part d'un `Instant` de la partie. Une télé rechargée, ou un téléphone revenu, la reprend au point où elle en est. La révélation de Punchline à 0,8 s, 2 s et 3 s ne recommence pas à une reconnexion. Une vue arrivée trop tard pour une animation la montre finie.

Un compte à rebours est un composant du commun. Il se calcule à chaque image depuis l'échéance de l'état et l'heure estimée, qui ne recule jamais (SQ-35, SQ-36, CAT-99). Aucune page ne compte seule.

### 5.4 Valider ce qui s'affiche

Avant de dessiner, le commun vérifie la scène : valeurs finies, positions dans la scène et sa marge, nombre de nœuds, longueur des textes, clés uniques. Une scène invalide n'est pas dessinée : la précédente reste, et c'est une faute du jeu. La zone sûre de la télé appartient au commun (HIS-46).

### 5.5 Les effets

Un effet a une identité, un instant, un destinataire, une tenue et une péremption.

- **Identité.** Le commun garde, par destinataire, les identités déjà jouées dont l'instant n'est pas périmé. Une identité déjà jouée ne se rejoue pas : ni au réaffichage, ni à la reconnexion, ni au rejeu de la tête, ni au rejeu du journal (SQ-79, PUN-66, SAL-167). La mémoire reste bornée : au-delà de la péremption, l'identité s'oublie, et l'effet ne se jouerait plus de toute façon.
- **Instant.** Le commun programme l'effet à son instant, plus le retard mesuré de la sortie (son, enceinte, télé). Un effet arrivé après sa péremption est consommé sans être joué (SAL-155, SAL-176).
- **Tenue.** `Provisoire` : joué dès la tête. Si la tête ne le réémet plus avant que son pas devienne définitif, il est annulé : un son s'arrête, une vibration ne s'annule pas. `Definitive` : joué quand son pas est définitif. Une vibration de point marqué est toujours `Definitive`.
- **Destinataire absent.** L'effet est perdu, pas gardé pour plus tard.

Le commun préfixe chaque identité par le numéro de partie. Deux parties ne se confondent pas.

### 5.6 Les demandes

Une demande ne part que de la définitive. Une ligne de résultats ne s'écrit donc jamais deux fois, ni pour une manche que la fenêtre a refaite.

- Son identité est déterministe : le numéro de pas et son rang dans le pas.
- La réponse revient comme une entrée, placée au pas où elle arrive.
- Une demande annulée ou remplacée (`Groupe::Remplace`) ne rend plus rien au jeu : le commun jette sa réponse (TL-44 à TL-46, SAL-164).
- `NouvellePartie` crée une instance neuve, avec la `Garde` de l'ancienne et une graine neuve. L'ancienne mémoire disparaît. Une soirée ne garde donc que ce que le jeu garde exprès (CAT-73, CAT-75, OUT-145).
- `Oublier`, `Retirer`, `Fermer` sont trois choses. Un appareil oublié revient comme un neuf. Un appareil retiré le lit, et peut revenir comme un neuf. Une salle fermée refuse les nouveaux appareils, avec un refus typé.
- Un échec d'écriture revient comme une réponse. Le jeu l'annonce sans s'arrêter.

---

## 6. Bots, invariants et l'épreuve

### 6.1 Ce que le jeu donne

```rust
impl<'a, J: Jeu> Bot<'a, J> {
    pub fn style(&self) -> Style;                 // Sage, Hostile, Lent
    pub fn hasard(&mut self) -> &mut Hasard;      // le sien, pas celui du jeu
    pub fn regard(&self) -> &Regard;              // son téléphone
    pub fn ecran(&self) -> &Regard;               // ce que la télé montre à tous
    pub fn depuis_vue(&self) -> Duree;            // depuis que sa vue a changé
    pub fn geste(&mut self, g: J::GesteTelephone);
    pub fn tenir(&mut self, t: J::TenuTelephone);
    pub fn action(&mut self, a: J::ActionEcran);  // bot de l'écran seulement
    pub fn rien(&mut self);
}

impl<J: Jeu> Attente<J> {
    pub fn joueur(&mut self, a: Appareil, exemples: &[J::GesteTelephone]);
    pub fn ecran(&mut self, exemples: &[J::ActionEcran]);
    pub fn echeance(&mut self, p: NumPas);
    pub fn libre(&mut self, r: Raison);
}

impl<J: Jeu> Galerie<J> {
    pub fn cas(&mut self, nom: &'static str, etat: J);
}
```

- Un bot de téléphone lit sa vue, et l'état à travers son regard et celui de la télé. Il ne voit aucun secret de plus qu'un humain qui tient ce téléphone et regarde la télé.
- Un bot est une fonction : sa vue, son hasard, le temps depuis sa vue. Pas de fil, pas de verrou (SQ-85).
- `jeu_bot` tourne dans le module joué. Les bots sont le même binaire que la partie.

### 6.2 Les invariants

```rust
jeu::exiger!(v, personne_hors_du_sol(self), "personne ne sort du sol", "joueur {}", i);
```

`invariants` porte sur un état. `transition` porte sur un pas : l'état d'avant, l'entrée, l'état d'après (« une relance ne retire jamais un point »). `Verif` donne `autorite()` : un invariant lit les secrets.

### 6.3 Comment l'épreuve s'en sert

| Pression | Ce qu'elle fait | Ce qu'elle lit du jeu |
|---|---|---|
| Parcours | bots au nombre maximal de joueurs, plus un organisateur, jusqu'à la fin, puis `NouvellePartie`, puis une autre fin | `phase`, `fin`, `places` |
| Couverture | chaque `Phase` atteinte ; sinon le rapport nomme celle qui manque | `phase` |
| Jamais figée | après chaque pas, pour chaque attendu, essayer ses exemples sur une copie ; au moins un accepté, ou une échéance (CAT-78) | `attente` |
| Lois communes | à chaque pas : refus sans trace, lecture sans trace, vue valide, limites | rien |
| Invariants | à chaque pas sur la définitive ; `transition` sur chaque entrée acceptée | `Epreuve` |
| Invariant qui n'observe rien | perturber l'état par sa forme canonique (bornes, signes, ordres, suppressions) ; un invariant qui ne rougit jamais est refusé (HIS-02, HIS-03) | dérive `Etat` |
| Fuite de secret | chercher les octets de chaque valeur voilée dans les vues et projections des autres regards (CAT-46) | voiles |
| Équité | rejouer avec d'autres identités exposées ; comparer les vues, permutées (CAT-27) | rien |
| Prédiction | jouer chaque partie avec et sans prédiction, même script de gestes ; comparer le journal définitif | `previsible` |
| Réseau | coupure, silence sans fermeture, trou noir, paquet retenu puis livré, gigue en rafales, repli WebSocket, redémarrage de l'autorité, rechargement, onglet jumeau, pendant la poignée de main comprise (OUT-26, OUT-75 à OUT-79) | rien |
| Temps | gels de 0 à 10 min, découpes d'images différentes, réveil d'onglet ; même temps, même partie (CAT-14, CAT-15) | rien |
| Entrées | appuis de 0 à 3 pas, doublons, répétitions, états en retard de 0 à 3 s, gestes au-delà de la fenêtre, axes absurdes avant décodage | rien |
| Soirée | six heures simulées à vitesse machine ; octets vivants du tas et tables du commun, comptés (OUT-84, SQ-76) | `jeu_memoire` |
| Pires cas | chaque cas de chaque `Phase`, sous plusieurs formats d'écran : débordement, sortie, chevauchement, taille de texte | `pires_cas` |
| Latence | rejouer avec et sans un geste ; les nœuds qui diffèrent sont son effet exact ; mesurer jusqu'à leur première présentation (OUT-30) | rien |
| Mutants | muter le code du jeu ; rapport sans seuil, jugé par un humain (OUT-95) | ses tests |

Un contrôle qui n'a pas tourné n'est pas une réussite. Le rapport distingue sans objet, pas exécuté, échec du jeu et panne de l'outil (OUT-01, OUT-02, HIS-05, HIS-06). Chaque échec garde sa graine, son journal et l'entrée isolée.

Une attente qui expire dit ce qu'elle voyait : dernière valeur, durée, nombre d'images (OUT-94). La touche `F8` de l'ordinateur de la soirée pose une marque au journal pendant une vraie partie (OUT-141) ; la marque devient un rejeu.

### 6.4 Les identités permutées

Le commun crée seul les appareils. Il tient une identité interne, qui fixe l'ordre des gestes simultanés avec la graine. Il expose au jeu une renumérotation tirée par partie. L'épreuve change la renumérotation, rejoue, et compare : à renumérotation près, les vues doivent être les mêmes. Un jeu qui range ses joueurs dans un `BTreeMap<Appareil, _>` et qui en tire une règle échoue. Un jeu qui garde son propre ordre d'arrivée passe.

---

## 7. Les trois jeux, en esquisse

Ces esquisses montrent la forme, pas le jeu entier.

### 7.1 Sidequest

```rust
use jeu::prelude::*;

const P: Periode = Periode::hz(60);
const FRAICHEUR: Duree = Duree::ms(450, P);
const AGE_MAX: Duree = Duree::ms(1_000, P);
const ENTRE_SAUTS: Duree = Duree::ms(750, P);
const BULLE: Duree = Duree::ms(4_000, P);
const GRACE: Duree = Duree::ms(90_000, P);

#[derive(Etat)]
pub struct Sidequest {
    phase: Phase,
    phase_depuis: NumPas,
    pause: Option<NumPas>,
    /// Dans l'ordre d'arrivée. C'est la règle des collisions : jamais l'ordre des `Appareil`.
    persos: Vec<Perso>,
    prochain_rang: u32,
    points: BTreeMap<Appareil, Points>,
    paquet: Paquet,
    curseur: Curseur,
    manche: NumManche,
    echeance: Option<NumPas>,
    resultat: Option<Resultat>,
    chat: Ouvert,
    transitions: u64,
}

#[derive(Etat)]
struct Perso {
    appareil: Appareil,
    rang: u32,
    prenom: Prenom,
    look: Look,
    pos: V2,
    vit: V2,
    dir: Axe2,
    en_ligne: EnLigne,
    parti_a: Option<NumPas>,
    coupe_pendant: Option<NumManche>,
    dernier_saut: Option<NumPas>,
    bulle: Option<(Texte<80>, NumPas)>,
}

#[derive(Message)]
pub enum ActionEcran { Jouer, Suivant, Precedent, FinManche, Salon, Terminer, Rejouer, Pause(Choix), Chat(Choix), Libre(QuestionBrute) }

#[derive(Message)]
pub enum GesteTelephone { Rejoindre { prenom: Prenom, look: Look }, Saut, Message(Texte<80>) }

#[derive(Message, PartialEq)]
pub struct Direction(pub Axe2);

impl Tenu for Direction {
    type Cle = ();
    fn cle(&self) {}
    fn neutre() -> Option<Self> { Some(Direction(Axe2::ZERO)) }
    fn est_neutre(&self) -> Neutre { self.0.neutre() }
}

impl Jeu for Sidequest {
    type Config = ConfigSidequest;
    type ActionEcran = ActionEcran;
    type GesteTelephone = GesteTelephone;
    type TenuTelephone = Direction;
    type Refus = RefusSidequest;
    type VueTelephone = VueManette;
    type Phase = Phase;
    type Garde = GardeSidequest; // personnages, numéro de manche, compteur de rangs
    type Precedent = Aucun;

    fn declarer() -> Declaration<Self> {
        Declaration {
            nom: "sidequest",
            version_etat: 1,
            places: Places { joueurs: 0..=71, appareils: 72 },
            temps: Temps::PasFixes { periode: P, rattrapage_max: 3 },
            fenetre: Applicable::Oui(Duree::pas(6)),
            antidatage_max: Duree::ms(30, P),
            pause: Pause::DuJeu,
            ecran: FormeEcran::Monde { largeur: 1280, hauteur: 760, zone_sure: Pourcent(90) },
            telephones: Applicable::Oui(Telephones {
                tenu: ReglesTenu {
                    fraicheur: Applicable::Oui(FRAICHEUR),
                    age_max: Applicable::Oui(AGE_MAX),
                    debit_max: 30,
                },
                prediction: Applicable::SansObjet(Raison("les téléphones ne montrent pas la salle")),
                capacites_utiles: &[Capacite::Vibration, Capacite::Portrait, Capacite::PrenomApp, Capacite::PadNatif],
            }),
            manettes: Applicable::SansObjet(Raison("pas de manette aujourd'hui")),
            hasard: Applicable::SansObjet(Raison("aucun hasard dans les règles")),
            fin: Applicable::Oui(()),
            limites: Limites { geste_octets: 512, tenu_octets: 32, file_evenements: 64, vue_octets: 4_096, effets_par_pas: 80, memoire_octets: 32 << 20 },
            touches_du_commun: TouchesDuCommun { marque: Touche::F8, diagnostic: Touche::Lettre('d') },
            mecaniques: &[Mecanique::TempsReel, Mecanique::Physique, Mecanique::Texte],
        }
    }

    fn creer(c: Creation<Self>) -> Result<Self, RefusSidequest> {
        let paquet = Paquet::valider(&c.config.paquet)?; // un paquet invalide empêche de lancer
        Ok(Sidequest::neuf(paquet, c.garde, c.presents, c.pas))
    }

    fn appliquer(avant: Avant<'_, Self>, entree: Entree<Self>) -> Issue<Self> {
        match entree {
            Entree::Pas(p) => {
                if avant.lire().rien_a_faire(p.numero()) {
                    return avant.ignorer();
                }
                let mut e = avant.accepter();
                let (j, _, s) = e.parts();
                j.echeance_atteinte(p.numero(), p.heure_murale(), s); // compte la manche
                j.piloter();
                j.separer();
                j.oublier_les_absents(p.numero(), s);                  // Demande::Oublier après 90 s
                e.fin()
            }
            Entree::Tenu(c) => {
                let j = avant.lire();
                let Some(i) = j.perso_en_ligne(c.de) else { return avant.ignorer() };
                if j.dir_de(i) == c.valeur.0 { return avant.ignorer(); }
                let mut e = avant.accepter();
                e.persos_mut(i).dir = c.valeur.0; // l'âge et le silence sont déjà réglés par le commun
                e.fin()
            }
            // Motif courant : décider sur la lecture (une valeur possédée), puis l'appliquer à `avant`.
            Entree::Geste(g) => match g.geste {
                GesteTelephone::Saut => {
                    let j = avant.lire();
                    let Some(i) = j.perso_en_ligne(g.de) else { return avant.ignorer() };
                    if j.pause.is_some() || !j.saut_permis(i, g.quand.place.pas) {
                        return avant.ignorer();
                    }
                    let mut e = avant.accepter();
                    e.persos_mut(i).dernier_saut = Some(g.quand.place.pas);
                    e.fin()
                }
                GesteTelephone::Message(t) => avant.lire().message(g.de, t, g.quand).appliquer_a(avant),
                GesteTelephone::Rejoindre { prenom, look } => avant.lire().rejoindre(g.de, prenom, look).appliquer_a(avant),
            },
            Entree::Ecran(DeEcran::Action { action, quand }) => {
                // Une commande qui fait avancer la partie ne vaut que pour la phase qu'elle a vue.
                if action.avance_la_partie() && quand.vu < avant.lire().phase_depuis {
                    return avant.ignorer();
                }
                avant.lire().commande(action, quand).appliquer_a(avant)
            }
            Entree::Presence(p) => avant.lire().presence(p).appliquer_a(avant),
            Entree::Reponse(r) => avant.lire().reponse(r).appliquer_a(avant),
            Entree::Ecran(_) | Entree::Manette(_) => avant.ignorer(),
        }
    }

    fn vue_ecran(&self, r: &Regard, s: &mut Scene<ActionEcran>) {
        for p in self.persos.iter().filter(|p| p.en_ligne.oui()) {
            s.noeud(Cle::de(&("perso", p.rang)), self.sprite(p)).glisse().pixel_entier();
        }
        if let Some(e) = self.echeance {
            let c = match self.pause {
                None => CompteARebours::Court { echeance: e, arrondi: Arrondi::SecondeAuDessus },
                Some(depuis) => CompteARebours::Fige { restant: e.depuis(depuis), arrondi: Arrondi::SecondeAuDessus },
            };
            s.compte_a_rebours(Cle::de(&"temps"), c);
        }
        if let Some(res) = &self.resultat {
            s.definitif(Cle::de(&("resultat", res.manche)), |s| self.dessiner_comptes(res, s));
        }
        self.barre_organisateur(r, s); // boutons et raccourcis, chacun nomme une ActionEcran
    }

    fn vue_telephone(&self, r: &Regard) -> VueManette { self.manette_de(r) }
    fn phase(&self) -> Phase { self.phase }
    fn attente(&self, a: &mut Attente<Self>) {
        match self.phase {
            Phase::Question => a.echeance(self.echeance_ou_maintenant()),
            Phase::Salon | Phase::Resultats => a.ecran(&[ActionEcran::Suivant]),
            Phase::Fin => a.ecran(&[ActionEcran::Rejouer]),
            Phase::Accueil => a.ecran(&[ActionEcran::Jouer]),
        }
    }
    fn garder(&self) -> GardeSidequest { self.garde() }
    fn convertir(a: Aucun) -> Result<Self, RefusSidequest> { a.jamais() }
}
```

Ce que l'esquisse montre :

- **Le pilotage** lit `dir` dans l'état. Le commun a déjà lâché une direction trop vieille (plus d'1 s) ou muette (plus de 450 ms). Un stop passe toujours. Le jeu ne garde plus d'heure de réception (SQ-01, SQ-02, CAT-25).
- **Les « relâchements forcés »** du cahier deviennent : pendant la pause, les Résultats et la Fin, `piloter` ne lit pas `dir`. Un pouce resté appuyé repart à la reprise. Le cahier dit qu'une reprise relâche : il faut trancher. Cette proposition penche pour garder la direction, comme au début d'une manche.
- **L'ordre des collisions** est `persos`, dans l'ordre d'arrivée, et le départage de deux corps superposés se fait par `rang`, pas par `Appareil` (SQ-71, CAT-27). Le cahier dit « l'identifiant de i » : c'est le rang du jeu.
- **La croix** envoie (1, 1). Le décodage d'`Axe2` le ramène à une norme 1. La normalisation du jeu ne fait plus rien. Les trajectoires de référence restent à vérifier à 1e-6 près : les derniers bits de la norme dépendent de la formule, et une seule doit exister.
- **La fenêtre** de 6 pas place un relâchement à son vrai moment. La glissade d'arrêt mesurée (26,5 px avec 60 ms par sens) peut baisser : le personnage s'arrête où il se serait arrêté sans réseau, puis le lissage cache la correction. À mesurer, et à juger sur la vraie télé.
- **Le résultat** d'une manche est `definitif`. La télé passe aux Résultats tout de suite, les comptes et les « +1 » arrivent 100 ms après. La ligne de résultats part de la définitive seule, une fois.
- **Le son** de phase est un effet d'identité `("phase", transitions)`, `Provisoire`, pour l'écran, périmé après 250 ms. **La vibration** de point gagné est `Definitive`, pour l'appareil du gagnant, d'identité `("point", manche, appareil)` : celui qui avait déjà le point n'en reçoit pas.
- **L'oubli** après 90 s est une `Demande::Oublier`. Le jeu décide ; le commun ne libère jamais une place seul.
- **La pause** est au jeu : elle a ses règles (échéance repoussée, messages permis).
- **Rejouer** est `Demande::NouvellePartie`. La `Garde` porte les personnages, le compteur de rangs et le numéro de manche. Les points ne passent pas.
- **Les téléphones** reçoivent leur `VueManette`, calculée sur l'ordinateur de la soirée. Ils ne reçoivent ni le module, ni la salle.

Ses bots :

```rust
impl Epreuve for Sidequest {
    fn bot_telephone(etat: &Self, vue: &VueManette, b: &mut Bot<'_, Self>) {
        match vue {
            VueManette::Selecteur { .. } => b.geste(GesteTelephone::Rejoindre { prenom: prenom_de_bot(b), look: look_de_bot(b) }),
            VueManette::Manette { .. } => match etat.zone_visee(b.regard(), b.hasard()) {
                Some(cible) => b.tenir(Direction(etat.vers(b.regard(), cible))),
                None => b.tenir(Direction(Axe2::ZERO)),
            },
        }
        if b.hasard().une_sur(jeu::non_nul!(30)) == Tirage::Oui { b.geste(GesteTelephone::Saut); }
    }
    fn bot_ecran(etat: &Self, b: &mut Bot<'_, Self>) {
        match etat.phase {
            Phase::Accueil => b.action(ActionEcran::Jouer),
            Phase::Salon | Phase::Resultats if b.depuis_vue() > Duree::ms(2_000, P) => b.action(ActionEcran::Suivant),
            Phase::Fin => b.action(ActionEcran::Rejouer),
            Phase::Salon | Phase::Resultats | Phase::Question => b.rien(),
        }
    }
    fn invariants(&self, v: &mut Verif<'_>) {
        for p in &self.persos {
            jeu::exiger!(v, p.pos.dans(SOL_MOINS_RAYON), "personne ne sort du sol", "rang {}", p.rang);
            jeu::exiger!(v, p.vit.norme() <= 218.5, "vitesse plafonnée", "rang {}", p.rang);
        }
    }
    fn transition(avant: &Self, _e: &Entree<Self>, apres: &Self, v: &mut Verif<'_>) {
        jeu::exiger!(v, apres.points_jamais_retires(avant), "une relance ne retire pas de point", "");
    }
    fn pires_cas(p: Phase, g: &mut Galerie<Self>) {
        match p {
            Phase::Fin => g.cas("fin, 71 prénoms de 18 lettres larges, 9 999 points", Sidequest::pire_fin()),
            Phase::Question => g.cas("question de 180 signes, 4 réponses de 36", Sidequest::pire_question()),
            Phase::Resultats => g.cas("comptes à deux chiffres et « · +1 » partout", Sidequest::pires_resultats()),
            Phase::Salon => g.cas("71 bulles de 80 signes", Sidequest::pire_salon()),
            Phase::Accueil => g.cas("accueil", Sidequest::accueil()),
        }
    }
}
```

Les coupures et les retours ne sont pas écrits par le bot : l'épreuve les injecte, sur le vrai chemin du lien. L'invariant 7 du cahier (« une coupure ne change pas le résultat ») devient une propriété de l'épreuve : rejouer avec et sans coupures courtes, et comparer les résultats, sous les hypothèses que le cahier écrit (PUN-73).

### 7.2 Punchline

```rust
#[derive(Etat)]
pub struct Punchline {
    phase: Phase,
    phase_depuis: NumPas,
    echeance: Option<NumPas>,
    sieges: Vec<Siege>,
    hasard: Hasard,
    paquets: Paquets,
    manche: Manche,
    duels: Vec<Duel>,
    courant: usize,
}

#[derive(Etat)]
struct Cote {
    /// Caché pendant le vote, montré à la révélation.
    auteur: Voile<IdJoueur>,
    /// Vu de son auteur pendant l'écriture ; de tous au vote.
    reponse: Voile<Option<Reponse>>,
    /// Vu de son auteur seul, jamais montré.
    brouillon: Voile<Option<Texte<80>>>,
}

#[derive(Etat)]
struct Duel { consigne: Consigne, gauche: Cote, droite: Cote, bulletins: Voile<Vec<Bulletin>> }

#[derive(Message, PartialEq)]
pub struct Brouillon { pub consigne: IdConsigne, pub case: Borne<u8, 0, 2>, pub texte: Texte<80> }

impl Tenu for Brouillon {
    type Cle = (IdConsigne, Borne<u8, 0, 2>);
    fn cle(&self) -> Self::Cle { (self.consigne, self.case) }
    fn neutre() -> Option<Self> { None } // un brouillon ne se relâche jamais seul
    fn est_neutre(&self) -> Neutre { Neutre::Non }
}
```

- `fenetre: SansObjet("tour par tour : vu suffit")`, `pause: Aucune("chaque phase a une échéance")`, `hasard: Oui`, `fin: Oui`.
- `ReglesTenu { fraicheur: SansObjet("un brouillon reste"), age_max: SansObjet("compté s'il arrive avant l'échéance"), debit_max: 5 }`.
- **Le brouillon** est un état tenu par consigne. Il remonte au fil de la frappe, le dernier gagne, à débit borné (PUN-01, PUN-03). Il est rangé dans un voile vu de son seul auteur. À l'échéance, l'étape le recopie en réponse. Un brouillon d'une autre phase est ignoré sur `vu` (PUN-04).
- **« Je sèche »** tire une réponse de secours après `accepter`. Refusé en finale avant `accepter` : le générateur n'a pas bougé (PUN-27). C'est écrit par la forme, pas par discipline.
- **Les secrets** suivent le cahier : réponses vues de leur auteur pendant l'écriture, montrées à tous au vote ; auteurs cachés au vote, montrés à la révélation ; bulletins cachés jusqu'à la révélation ; chaque téléphone ne voit que ses consignes. La télé passe par le regard `Ecran` : pendant le vote, `auteur.voir(r)` rend `None` sur la télé aussi.
- **Les absences** (20 s, présents plus nombreux) sont des règles du jeu, sur les `Presence` et leurs pas. Le siège se libère au salon, après 90 s, par `Demande::Oublier`.
- **Les robots** de Punchline sont des joueurs du jeu, sans appareil. Ils agissent à `phase_depuis + 1,5 s`, au pas exact. Un réveil tardif ne change rien : le temps glisse, il ne saute pas (PUN-35).
- **Le son** de phase : `IdEffet::de(&("phase", numero_de_transition))`, `Provisoire`, sans fenêtre donc joué tout de suite. Une télé rechargée repart en silence.
- **Le compte à rebours** des téléphones est le composant du commun, depuis l'échéance : juste après une reconnexion, sans minuteur de page.
- **Les bots d'épreuve** : le VIP lance dès trois joueurs, chacun écrit (textes hostiles en style `Hostile`, avec des sentinelles), sèche parfois, vote au hasard après 1,5 s ; l'organisateur passe, ajoute un robot, relance. Le commun cherche les sentinelles dans chaque vue où elles devraient manquer.

### 7.3 Tier list

```rust
#[derive(Etat)]
pub struct TierList {
    plateau: Plateau,
    histoire: Histoire<Plateau, 200>,
    glisser: Option<Glisse>,
    operations: BTreeMap<IdDemande, Operation>,
    avis: Option<Avis>,
}

#[derive(Etat)]
enum Contenu { Image(Partage<[u8]>), Texte(Texte<60>) }
```

- `telephones: SansObjet("jeu solo par décision de l'hôte")`, `manettes: SansObjet(…)`, `fenetre: SansObjet("une seule machine")`, `hasard: SansObjet(…)`, `fin: SansObjet("un éditeur n'a pas de fin")`, `pause: Aucune(…)`.
- `temps: ALaDemande { periode: 60 Hz, rattrapage_max: 3 }`. `besoin_de_pas` rend `Oui` pendant un glisser, pour le défilement automatique, et `Non` sinon. La vitesse du défilement se compte par seconde, en pas (TL-41, TL-42).
- **Les entrées** sont toutes de l'écran : composants du commun (déposé, texte édité, fichiers reçus), pointeur et clavier bruts. Le raccourci Ctrl+Z demande la répétition : `clavier_brut(ClavierBrut::AvecRepetition)` (CAT-17).
- **L'histoire** garde des plateaux entiers. `Partage<[u8]>` partage les images entre les étapes (TL-20). La forme canonique écrit chaque `Partage` une seule fois.
- **Les imports et l'export** sont des demandes. Les imports sont `Independante`. L'export et la restauration sont `Remplace(Cle::de(&"export"))`. Reset annule tout par `Demande::Annuler`. Une réponse qui arrive après ne touche à rien (TL-44 à TL-46).
- **Rechargement de la page** : le point de reprise et le journal rendent le plateau et l'histoire. Un second onglet sur la même partie devient un second regard sur l'écran, en lecture seule, qui peut reprendre la main d'un geste. À décider avec l'hôte.
- **`attente`** rend `libre(Raison("un éditeur attend l'écran, sans progression"))`. L'épreuve n'y cherche pas de fin, mais vérifie que l'écran a toujours une action acceptée.
- **Bots** : style `Hostile` seulement. Suites d'actions au hasard, valides ou non, textes hostiles, fichiers abîmés, annulations.

### 7.4 Un FPS, pour éprouver l'interface

Aucun jeu de la soirée n'en est un. L'esquisse sert à trouver ce qui casserait.

```rust
#[derive(Message, PartialEq)]
pub struct Commandes {
    pub deplacement: Axe2,
    /// La visée cumulée depuis l'arrivée, absolue : un paquet perdu ne la décale pas.
    pub visee: Cumul2,
    pub tir: Bouton,
    pub saut: Bouton,
}

#[derive(Etat)]
struct Joueur {
    pos: V3, vit: V3, cap: Angle, site: Angle,
    derniere_visee: Cumul2,
    tirs_vus: Cumul,
    /// Les positions des derniers pas, pour juger un tir sur ce que le tireur voyait.
    passe: Anneau<V3, 16>,
}

#[derive(Etat)]
struct Arene {
    joueurs: Vec<Joueur>,
    /// Une position que seuls voient ceux qui ont une ligne de vue : pas de vision à travers les murs.
    caches: BTreeMap<IdJoueur, Voile<V3>>,
}
```

- `fenetre: Oui(Duree::pas(12))`, `prediction: Oui(())`, `previsible` : le déplacement et le tir de l'appareil lui-même.
- **La visée** est un `Cumul2`, pas un écart. Le jeu lit `visee.depuis(derniere_visee)` à chaque changement. Une souris ou un gyroscope envoient leur somme, jamais un delta (REM-13).
- **Le tir** lit `tir.appuis.depuis(tirs_vus)`. Deux tirs dans un même envoi comptent deux.
- **Un tir se juge sur ce que voyait le tireur** : `quand.vu` et `quand.fait.fraction` désignent l'image de l'appareil, et le jeu relit `passe`. La fenêtre replace le tir à son pas ; l'anneau couvre l'interpolation de l'affichage. Le commun ne fait pas cette compensation : elle dépend de la mécanique.
- **La balle prédite et la balle confirmée** se nomment depuis `quand.id` (§ 4.6).
- **Le son du tir** est `Provisoire`, d'identité l'`IdGeste`. **Le marqueur de touche** est `Definitive`. Une élimination ne s'annonce qu'en `Definitif`.
- **Un ennemi caché** est un voile. Sur le téléphone, une prédiction qui le touche rend `Absent` : ce coup attend la confirmation. La vision à travers les murs est impossible par la forme.
- **Le coût** : 12 pas rejoués par image où un geste arrive en retard. L'épreuve vérifie `(12 + 2) × coût d'un pas` sous le budget d'image.

Ce qui aurait cassé sans ces choix :

| Sans | Le FPS casse ainsi |
|---|---|
| `Cumul` | une visée en écarts dérive à chaque paquet perdu |
| `Bouton.appuis` | un tir court entre deux envois disparaît |
| `IdGeste` | chaque tir prédit montre deux balles |
| `Definitif` | le tireur voit « éliminé », puis la correction le retire (CAT-41) |
| `Tenue` sur les effets | un son de tir rejoué à chaque rejeu de la tête |
| `touches_du_commun` déclarées | un diagnostic du commun sur `D` vole le déplacement à droite |
| voiles dynamiques | la projection envoie toutes les positions : vision à travers les murs |
| budget en instructions | une faute de budget ne se rejoue pas |

---

## 8. Ce que l'interface rend impossible

| Famille de bugs | Ce qui la rend impossible | Leçons |
|---|---|---|
| Un refus qui change l'état ou consomme le hasard | `Avant` ne donne que `&J` ; refuser n'existe plus après `accepter` | PUN-27, OUT-171, HIS-44, SQ-25 |
| Une valeur non finie ou hors bornes dans le jeu | les types de la frontière se valident au décodage | HIS-45, CAT-22, SQ-67, SAL-117, SAL-118 |
| Un octet restant lu de travers | un cadre se consomme en entier | SAL-118, SAL-119 |
| Un secret dans la vue d'un appareil, télé comprise | la vue n'a pas le jeton ; elle voit à travers son regard | CAT-46, SQ-64 |
| Une vision à travers les murs | la projection vide les voiles qu'un regard ne voit pas | CAT-46 |
| Une échéance qui survit à sa partie | aucun minuteur ; une nouvelle partie est une instance neuve | CAT-75, CAT-73, CAT-71 |
| Un temps compté en images | le jeu ne voit que des pas | CAT-14, TL-41 |
| Une spirale après un gel, des phases traversées d'un coup | plafond de rattrapage ; le temps perdu est jeté, les pas ne sautent pas | CAT-15, SQ-62, PUN-34 |
| Un appui perdu ou compté deux fois | événements comptés une fois ; compteur d'appuis dans l'état tenu ; répétition écartée | CAT-16, CAT-17, REM-16, REM-17, SQ-99 |
| Un vieux geste qui fait bouger | âge maximal et fraîcheur au commun ; stop qui passe toujours | SQ-01, SQ-02, CAT-25 |
| Un geste d'une phase passée appliqué à la suivante | `vu` sur chaque geste ; placement dans la fenêtre | CAT-44, SAL-115, PUN-38, SQ-23 |
| Un appareil qui antidate | `fait` borné par la latence mesurée | SAL-129 |
| Un effet rejoué par un réaffichage, une reconnexion, un rejeu | identité d'effet, mémoire bornée par la péremption | SQ-79, PUN-66, SAL-167 |
| Une vibration pour un absent, jouée en retard | péremption ; destinataire absent, effet perdu | SAL-155, SAL-176 |
| Un résultat montré puis retiré | `Definitif` vient de la définitive, par clé | CAT-41 |
| Une ligne de résultats écrite deux fois | les demandes ne partent que de la définitive | CAT-76 |
| Une réponse de demande annulée qui touche l'état | le commun jette la réponse | TL-44, TL-45, TL-46, SAL-164 |
| Un téléphone qui envoie une commande d'organisateur | un type de geste par rôle ; l'écran est la seule machine qui produit `ActionEcran` | SQ-21, SQ-22, SAL-33 |
| Un jeu qui manie liens, sessions ou jetons | il ne voit que des `Appareil` | SAL-08, SAL-14, SAL-15 |
| Une exemption silencieuse | `Applicable` sans `Default`, raison exigée | HIS-04, OUT-03, OUT-04 |
| Une soirée qui accumule la mémoire | instance neuve par partie ; `Oublier` décidé par le jeu | SQ-76, PUN-15, OUT-145 |
| Une panique qui tue la salle | faute isolée, restaurée, rejouée sans l'entrée | SAL-140 |
| Une vue datée de l'instant d'arrivée | une vue est celle d'un pas | SQ-86 |
| Un affichage remis à zéro par une reconnexion | la vue se calcule depuis l'état, identités comprises | REM-12 |
| Un ordre d'itération qui diffère entre natif et module | `Etat` n'existe pas pour `HashMap` ; NaN canoniques | SQ-68, CONTRAT |
| Une sauvegarde écrite à la main, désaccordée | forme canonique dérivée ; conversion explicite | HIS-51 |
| Un compte à rebours qui recule ou qui dérive | composant du commun depuis l'échéance et l'heure estimée | SQ-35, SQ-36, CAT-99 |
| Des bots qui se bloquent entre eux | un bot est une fonction dans le module | SQ-85 |
| Des limites différentes de chaque côté | limites de l'autorité dans la déclaration, dites à l'accueil | SAL-121, SAL-07 |
| Une prédiction qui décide | la définitive ne reçoit que des gestes | SAL-194, CONTRAT |
| Une faute de budget qui ne se rejoue pas | budget compté en instructions | SQ-59, SQ-60 |

Ce que la forme n'empêche pas, et qui reste sous pression :

- copier un secret dans un champ public, depuis l'étape : recherche des octets voilés (§ 6.3) ;
- résoudre deux actions simultanées l'une après l'autre : question de mécanique, le jeu rassemble les gestes d'un pas s'il veut l'équité (CAT-28) ;
- un `Partage` cloné dans une lecture : empreinte avant et après pendant l'épreuve ;
- une règle d'équilibre fausse : les invariants du jeu, ses tests, et les mutants.

---

## 9. Risques et questions ouvertes

### 9.1 Risques

1. **Le coût de la fenêtre.** Une copie de mémoire et `fenêtre` pas par image en retard. Sidequest : 2 Mio et 6 pas, quelques dixièmes de milliseconde attendus. Un FPS avec une physique tierce : inconnu. À mesurer avant de choisir les fenêtres du § 4.7.
2. **Le compteur d'instructions et les NaN canoniques** ralentissent le module. Personne n'a mesuré de combien sur nos jeux. Si c'est trop, on peut ne canoniser qu'aux sorties de l'étape et aux points de reprise : à prouver équivalent.
3. **Les gros états.** Tier list : des dizaines de Mio. Un point de reprise copie tout. Sans suivi des pages modifiées, il coûte une image entière. Le suivi demanderait une réécriture de chaque écriture mémoire, ou une seconde mémoire (Wasm 3.0) dont le support dans Safari reste à vérifier.
4. **L'ergonomie d'`Avant` et `Apres`.** Le vérificateur d'emprunts va gêner : lire, garder ce qu'on a lu, puis accepter. `parts()` aide. Les noms d'`Apres` peuvent masquer ceux du jeu. À éprouver sur le modèle, écrit par un agent qui n'a pas vu ce document.
5. **L'extrapolation des autres joueurs** sur un appareil qui prédit. Le contrat dit que seules les conséquences des gestes de l'appareil s'affichent avant confirmation. L'étape, elle, fait avancer tout le monde. Cette proposition l'accepte comme « prédire le suivant », et laisse `Definitif` à qui veut le modèle du Source Engine. Le mainteneur doit trancher.
6. **Le glissement du temps** change une phrase du cahier de Punchline : après un gel, la phase en cours continue au lieu de se terminer. Le temps perdu est bien jeté. À corriger dans le cahier si on garde ce choix.
7. **Les relâchements forcés** de Sidequest changent de sens (§ 7.1). À trancher par l'hôte.
8. **La forme d'un voile fuit** : son existence, sa taille, qui le voit. Un jeu qui voile chaque réponse dit combien il y en a. À écrire dans le modèle, avec un exemple.
9. **`vu` peut mentir** vers le passé : l'appareil fait alors refuser ses propres gestes. Il ne peut pas mentir vers le futur au-delà de ce qu'on lui a envoyé. Le commun le borne.
10. **L'antidatage** reste possible jusqu'à la demi-latence mesurée plus la marge. Un téléphone qui retarde ses échos gagne quelques millisecondes dans une course de buzzer. Entre amis, on l'accepte ; on le dit.
11. **Une vibration provisoire** ne s'annule pas. Le modèle doit dire de les garder `Definitive`.
12. **La pile hors de l'état** suppose que rien ne relit une pile non initialisée. C'est vrai du Rust sûr. Le moteur le constate en comparant deux exécutions qui appellent les vues un nombre différent de fois.
13. **`postcard` et `serde`** sont du code tiers dans le module joué. Leur format est figé, et un décodeur fautif ne fait qu'une faute isolée. Ils restent à réduire un jour.
14. **Le moteur dans un worker** et la télé dans la page : deux horloges, un message entre elles. C'est exactement le cas où 15 à 28 % d'images restaient figées (SQ-56). La marge se mesure, sur la vraie télé.
15. **L'onglet de la partie qui fait foi en arrière-plan** : un worker est aussi ralenti par certains navigateurs. Le contrat demande de le garantir ou de le signaler. Rien ici ne le garantit encore.

### 9.2 Questions ouvertes

1. La durée de chaque fenêtre, par genre, mesurée sur un vrai Wi-Fi de soirée.
2. Faut-il un historique dans le commun pour juger un tir sur l'image du tireur, ou laisser chaque jeu garder son anneau ?
3. Les clés de fusion de `Definitif` : faut-il que la dérive les tire d'un champ marqué, plutôt que de les laisser écrire ?
4. Les effets provisoires : leur annulation doit-elle couper un son déjà parti, ou seulement empêcher de le rejouer ?
5. La page d'un appareil qui prédit : reçoit-elle la vue prédite et la vue confirmée, pour montrer « en attente » sous un geste ?
6. Un jeu doit-il pouvoir déclarer ses propres rôles (« équipe », « arbitre »), ou les trois rôles du commun suffisent-ils ?
7. Le SIMD flottant : interdit au départ. Une physique tierce le voudra.
8. Le suivi des pages modifiées : réécrire les écritures mémoire, ou attendre une seconde mémoire partout ?
9. Les touches que le commun se réserve : `F8` et une touche de diagnostic que le jeu choisit. Est-ce assez ?
10. Un second onglet sur la partie de Tier list : regard en lecture seule, ou refus ?
11. Le Component Model : quand il remplacera `postcard`, la forme canonique doit rester lisible pour les sauvegardes d'avant.
12. La preuve de terminaison : le compteur d'instructions donne un budget, pas une preuve. Le premier essai de preuve (« ne jamais planter la partie ») portera-t-il sur l'étape, ou sur la boucle du moteur ?
