# Le contrat de la cartouche

> Ce fichier s'adresse à toi, qui fais un jeu, et à l'agent de code qui l'écrit avec toi. `soiree contrat` l'affiche.
>
> **Il est fait pour ne plus bouger.** Il ne change que sur demande explicite de l'hôte, et par ajout : ce que la console apprend à faire s'appuie sur lui, sans qu'aucun jeu ait à changer.

## L'idée

La console est le programme. Le jeu est une cartouche : une **machine** (ses règles) et des **pages** (l'écran et la télécommande).

Chaque contrainte est posée à l'écriture, donne une propriété au jeu, et cette propriété fait disparaître une classe de bugs.

| Contrainte | Propriété | Classe de bugs évitée |
|---|---|---|
| 1. La machine se passe du système | Mêmes entrées, même partie | Le bug qui n'arrive qu'une fois |
| 2. Seule la machine décide | Tout ce qui compte est à l'endroit testable | La règle cachée dans le programme ou dans une page |
| 3. Une seule porte d'entrée | La console connaît tout ce qui peut arriver à la partie | L'action jamais testée, ou venue par un chemin imprévu |
| 4. L'état de la machine est une simple donnée | La partie se photographie, se compare, se reprend | L'état caché, impossible à inspecter ou à retrouver |

## Ce que le jeu fournit

### La machine

Un crate sans système (`#![no_std]`). Il exporte un type, `Jeu`, qui remplit le contrat de la brique `cartouche`, elle aussi sans système :

```rust
/// Une page : qui elle est, et ce qu'elle est. La même page garde le même
/// numéro quand elle se recharge ou revient de veille.
pub struct Page { pub id: u64, pub role: Role }   // Role::Screen ou Role::Remote

/// Tout ce qui peut arriver à la partie. Il n'y a pas d'autre porte, sauf
/// celle qu'une machine ouvre elle-même en demandant quelque chose au monde
/// extérieur (`asks`) : la réponse lui revient par `answer`.
pub enum Input<Command> {
    /// Une page arrive ou revient.
    Joined(Page),
    /// Une page n'est plus là.
    Left(Page),
    /// Une page a envoyé une commande.
    Message(Page, Command),
    /// Un pas de temps s'est écoulé.
    Tick,
}

/// À qui va un effet.
pub enum To { Page(Page), Screens, Remotes, Everyone }

pub trait Machine: State {
    /// Ce qu'une page peut envoyer.
    type Command: DeserializeOwned;
    /// Ce qu'une page doit montrer maintenant.
    type View: Serialize;
    /// Ce qu'une page joue une fois, et qu'on peut rater sans rien perdre.
    type Effect: Serialize;

    /// Le rythme du jeu. 0 : le temps ne compte pas.
    const TICKS_PER_SECOND: u32;
    /// Vrai : la console garde l'état d'un lancement à l'autre.
    const REMEMBERS: bool = false;
    /// Faux : le jeu se joue sans télécommande.
    const REMOTES: bool = true;

    /// Une partie neuve. La graine est tout le hasard du jeu. Ne peut pas échouer.
    fn new(seed: u64) -> Self;
    /// La seule façon de changer la partie.
    fn apply(&mut self, input: Input<Self::Command>) -> Vec<(To, Self::Effect)>;
    /// Ce que cette page doit montrer. Ne change rien.
    fn view(&self, page: Page) -> Option<Self::View>;
    /// Ce que cette page pourrait raisonnablement envoyer maintenant.
    fn suggest(&self, page: Page) -> Vec<Self::Command>;

    /// Ce que la machine demande au monde extérieur maintenant. Ne change rien.
    fn asks(&self) -> Vec<Ask> { Vec::new() }
    /// La console a répondu à la demande numéro `id`.
    fn answer(&mut self, id: u32, answer: Answer) -> Vec<(To, Self::Effect)> { Vec::new() }
    /// Ce qu'internet ou un contenu pourrait répondre à la demande numéro `id`.
    fn guess(&self, id: u32) -> Vec<Answer> { Vec::new() }
}

/// L'état de la machine s'écrit en octets et se relit. Rien à écrire quand
/// tout l'état dérive `Serialize` et `Deserialize` : la brique le fait.
pub trait State: Sized {
    fn save(&self) -> Vec<u8>;
    fn load(bytes: &[u8]) -> Option<Self>;
}

/// Le ticket d'un contenu lourd (une image, un son) que la console garde.
pub struct Content { pub print: u64, pub len: u32, pub kind: Kind }
pub enum Kind { Image, Sound, Video, Other }

/// Une demande au monde extérieur, sous le numéro que la machine lui donne :
///   Ask::date(id)                    la date et l'heure chez l'hôte
///   Ask::web(id, Web::post(url, octets).header(nom, valeur))   une question à internet
///   Ask::content(id, ticket)         les octets d'un contenu
///   Ask::save(id, nom, octets), Ask::load(id, nom), Ask::erase(id, nom), Ask::saves(id)
pub struct Ask { pub id: u32, .. }

/// La réponse, lue par la question posée : as_date(), as_web(), as_bytes(),
/// as_names(), is_done(). Ces lectures ne rendent rien quand le monde n'a
/// pas répondu.
pub struct Answer { .. }
```

Ce que chaque ligne impose, et pourquoi :

- **`State`** (contrainte 4). Tout l'état de la partie s'écrit et se relit, dans un format exact au bit près que la brique fournit. La console en tire l'empreinte, la photo et la reprise : le jeu n'écrit plus son empreinte à la main, un champ oublié devient impossible. Une machine qui porte un état qu'elle n'a pas écrit (une bibliothèque de physique) écrit `save` et `load` elle-même.
- **`Command`** (contrainte 3). Les pages l'envoient déjà. La console s'en sert pour tirer des commandes au hasard, absurdes comprises : un `usize` n'y a pas sa place, il n'a pas la même taille dans la page et sur l'ordinateur.
- **`new(seed)`**. Les données de départ (textes, cartes, réglages) sont compilées dans la machine ; un test vérifie qu'elles sont bonnes. Ce qui se choisit au salon est une commande.
- **`Tick`**. Le temps entre par la porte, à rythme fixe : la machine compte des pas, elle ne reçoit jamais l'heure. Une commande arrivée entre deux pas est datée du dernier.
- **`view`**. Une vue se calcule sans rien changer, et dit tout ce qui dure. Une page rechargée retrouve tout dans la vue suivante.
- **`Effect`**. Ce qui ne dure pas : un son, une vibration, un « trop tard ». `apply` les rend, la console les porte une fois. Une vérité n'est jamais un effet : une page absente à ce moment-là ne le reçoit pas.
- **`suggest`**. Ce que les boutons de cette page enverraient maintenant. La console s'en sert pour jouer des parties qui avancent ; une machine peut y mettre le coup d'un bon joueur.
- **`REMOTES`**. Un jeu qui se joue sans télécommande le dit. La console n'annonce pas d'adresse aux téléphones et n'en laisse entrer aucun : la machine ne voit jamais de `Role::Remote`, et n'a rien à écrire pour un visiteur dont elle ne veut pas.
- **`Content`** (contrainte 4). Ce qui est lourd, une image ou un son qu'un joueur apporte, ne voyage pas dans une commande : la page le confie à la console, qui rend un ticket. La commande, l'état, la vue et le journal ne portent que le ticket, et restent légers. La machine ne voit jamais les octets : une règle peut dépendre de la taille et du genre d'un contenu, pas de ce qu'il montre. Un ticket peut ne mener à rien (une page a envoyé n'importe quoi, ou le contenu est effacé) : la partie continue, la page laisse un vide.
- **`asks` et `answer`** (contraintes 1 et 3). La machine ne fait rien dehors : elle liste ce qu'elle veut. La console le fait, note la réponse au journal, et la remet par `answer`, où elle seule passe. Rejouée, la partie n'interroge donc ni internet ni le disque. Ce qu'une machine peut demander :
  - la date et l'heure, à la montre de l'hôte ;
  - une question à internet (une IA qui juge une réponse). Un secret n'est jamais dans la machine : elle écrit `{NOM}` dans un en-tête, et la console y met la clé que l'hôte lui a confiée ;
  - les octets d'un contenu dont elle porte le ticket (un niveau fabriqué ailleurs) ;
  - des sauvegardes, chacune sous un nom : garder, relire, oublier, lister.

  Une demande reste dans la liste jusqu'à sa réponse. La console la fait une fois ; pour la refaire, la machine lui donne un autre numéro. Le monde peut ne pas répondre (pas d'internet ce soir, un contenu effacé) : la réponse est vide, et le jeu continue sans. Ce qui revient vient du dehors : la machine le relit sans lui faire confiance.
- **`guess`**. Ce qu'internet ou un contenu pourrait répondre à une demande. La console s'en sert comme de `suggest`, pour jouer des parties où la réponse arrive. La date et les sauvegardes, elle les invente seule.
- Le salon, les places, qui a le droit de lancer ou d'arrêter : tout cela est dans la machine (contrainte 2).

À l'intérieur de la machine, tout est libre : l'organisation, la physique, les bibliothèques, tant qu'elles se passent du système.

### Les pages

Du Rust compilé pour le navigateur, libre dans sa forme. Une page reçoit des vues et des effets, envoie des commandes.

- **Elle ne garde aucune vérité.** Ce qui décide de la partie est dans la machine. Elle garde ce qui sert à montrer : une animation, un ralenti, un texte en cours de saisie, un réglage de confort.
- **Elle envoie le plus brut possible.** Elle peut interpréter ce que fait le joueur quand le débit ou la latence l'imposent (reconnaître un geste dans cent mesures de capteurs par seconde). Ce qu'elle interprète, la console ne le teste pas : c'est au jeu de le tester.
- **Elle peut emprunter le crate de la machine** pour ses types et ses constantes, pas pour décider à sa place.
- **Elle confie à la console ce qui est lourd** (`telecommande::web::upload`), met le ticket reçu dans sa commande, et montre un contenu par son adresse (`telecommande::web::content_url`). Elle le réduit d'abord à ce qu'elle montrera : une image à la taille de sa tuile.
- La console lui donne, à la connexion : l'adresse des téléphones (vide pour un jeu sans télécommande), le rythme du jeu, et avec chaque vue le numéro du pas. Un compte à rebours se dessine sans qu'on renvoie une vue à chaque pas. Ce numéro compte tous les pas que la machine a reçus depuis sa naissance : il ne repart pas de zéro quand la console relance un jeu dont elle garde l'état.

### Rien d'autre

Le programme du jeu tient en une ligne, la même pour tous, que `soiree check` vérifie :

```rust
fn main() -> std::process::ExitCode {
    telecommande::console::run::<machine::Jeu>(env!("CARGO_MANIFEST_DIR"))
}
```

## Ce que la console fournit

### Pendant que le jeu tourne

- Elle sert les pages, reçoit les téléphones, tire la graine, donne le temps.
- **Le temps** : `TICKS_PER_SECOND` pas par seconde. Après un ralentissement, elle rattrape au plus un dixième de seconde et laisse tomber le reste.
- **Les vues** : après chaque entrée, elle calcule la vue de chaque page présente et ne l'envoie que si elle a changé (ses octets écrits ne sont plus les mêmes), au plus soixante fois par seconde. Une page qui arrive reçoit toujours la sienne.
- **Les effets** : portés une fois, aux pages présentes.
- **Le journal** : la graine, chaque entrée et chaque réponse du monde extérieur, avec le numéro de son pas, dans un fichier par partie. `soiree rejouer` refait la partie à l'identique. Les vieux fichiers s'effacent tout seuls.
- **`REMEMBERS`** : elle écrit l'état dans le dossier du jeu quand il change et le relit au lancement. Un état qu'elle n'arrive pas à relire est mis de côté, jamais écrasé, et elle le dit.
- **`REMOTES`** : pour un jeu sans télécommande, elle ne donne ni adresse ni QR code aux téléphones. Celui qui arrive quand même lit que le jeu se joue sur le grand écran, la même page pour tous les jeux.
- **Les contenus** : elle range ce que les pages lui confient dans le dossier du jeu, une seule fois chacun, 16 Mo au plus, et le rend sous le nom de son ticket. Quand le dossier dépasse 500 Mo, les contenus montrés il y a le plus longtemps s'effacent.
- **Les demandes** : après chaque entrée, elle lit `asks` et fait ce qui est nouveau, 16 demandes au plus à la fois. Une réponse dont la machine ne veut plus (la demande a quitté sa liste) n'entre nulle part. Après un arrêt, une console qui garde l'état refait les demandes restées sans réponse.
  - **Internet** : une adresse en `https://`, ou `http://localhost` pour un service qui tourne chez l'hôte. 1 Mo au plus dans chaque sens, trente secondes pour répondre, trente questions par minute, aucune redirection suivie. Le secret `{NOM}` est la variable `SOIREE_SECRET_NOM` ; il ne part que chiffré, et vers l'adresse que nomme `SOIREE_SECRET_NOM_POUR`. Il n'entre ni dans la machine, ni dans l'état, ni dans le journal.
  - **Les sauvegardes** : dans le dossier du jeu, 100 au plus, 1 Mo chacune, un nom de 64 octets au plus.
  - **Un contenu** : le journal ne porte que son ticket. La partie ne se rejoue au-delà que tant que la console garde ce contenu ; sinon `soiree rejouer` le dit.

### Avant la soirée : `soiree check` joue

Le linteur reste, aussi dur. S'y ajoute un banc, le même pour tous les jeux, sans une ligne écrite par le jeu :

- il joue des parties au hasard : des écrans et des téléphones (des écrans seulement, pour un jeu sans télécommande) arrivent et partent, envoient des commandes suggérées ou tirées au hasard ; le temps passe par longues rafales, ou pas à pas devant des joueurs attentifs qui saisissent une suggestion qui ne vaut qu'un instant ;
- il répond à la place du monde extérieur, tôt ou tard : une date quelconque ; un internet qui répond ce que la machine suppose (`guess`), n'importe quoi, ou rien ; un contenu absent ou rempli de n'importe quoi ; des sauvegardes qu'il garde en mémoire, un disque parfois plein, une sauvegarde parfois abîmée ;
- il photographie la partie en route, la reprend depuis la photo, et vérifie qu'elle continue à l'identique ;
- il compare les empreintes en natif et en WebAssembly ;
- il calcule les vues de toutes les pages à chaque pas ;
- il mesure le temps de chaque pas : un pas tient dans la moitié de son temps ;
- un plantage, un écart ou un pas trop lent refuse le jeu, et dit quelle graine rejouer pour le voir.

Un jeu peut garder un exemple à lui, qui joue des parties écrites d'avance pour aller là où le hasard ne va pas : la console le lance en natif et en WebAssembly, comme aujourd'hui. Et ses tests restent les siens.

## Ce que le contrat ne fixe pas

- Ce qui se passe dans la machine et dans les pages.
- Ce que la console fera plus tard de ces propriétés : rattraper un plantage, reprendre une soirée après un arrêt, n'envoyer d'une vue que ce qui a changé. Aucun jeu n'aura à changer pour ça.
- Les portes à venir : une chose de plus qu'une machine peut demander (`Ask`), une lecture de plus sur la réponse (`Answer`). Ce seront des ajouts, pas des réécritures.
