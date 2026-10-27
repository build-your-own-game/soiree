# Catalogue des items de certification

> Pour l'agent de code qui écrit le `CERTIFICATION.md` d'un jeu. Ce catalogue rassemble des items de certification déjà rédigés, chacun tiré d'un bug réel, pour que ton jeu n'ait pas à les redécouvrir en pleine soirée.

## À quoi il sert

`CERTIFICATION.md`, à la racine du dépôt commun, fixe le plancher de **tous** les jeux : reconnexions, entrées hostiles, textes vicieux, mise en page de la TV, endurance. Le harnais et salon le vérifient sur chaque jeu. Ce catalogue ne répète rien de tout cela.

Chaque jeu a aussi son propre `CERTIFICATION.md` : ce que **ce jeu-ci** doit tenir, à cause de ses règles. Ces items-là portent sur la logique et la sensation du jeu : un vote qui compte deux fois, un joueur coincé dans un mur, une égalité donnée au joueur 1, une question qui revient. Beaucoup se répètent d'un jeu à l'autre. Le catalogue les a écrits une fois, chacun avec :
- ce qui ne doit jamais arriver, en une ligne ;
- le bug réel qui l'a montré, avec sa source ;
- une propriété testable sur la simulation (`crates/core`), sans navigateur ni réseau ;
- comment la générer en test de propriétés ;
- ce qu'il faut changer pour ton jeu.

Le réseau n'y est jamais : les coupures, les jetons, les paquets perdus ou hostiles sont l'affaire de `salon/`, que les jeux ne réécrivent pas.

## Comment choisir les items de ton jeu

1. **Repère les mécaniques de ton jeu.** Relis ses règles (`LEARNINGS.md`, le code de `crates/core`) et coche ce qu'il fait dans la table ci-dessous. Un jeu en coche presque toujours plusieurs : un quiz au buzzer coche « vote et quiz », « égalités », « hasard », « texte » et « affichage ».
2. **Lis les fichiers correspondants en entier.** Les titres de l'index ne suffisent pas : un item s'applique souvent sous un autre nom (« consigne » chez toi s'appelle peut-être « mot » ou « défi »).
3. **Prends tout ce qui s'applique.** Dans le doute, prends l'item : un test de trop coûte quelques secondes, un bug en pleine soirée coûte la soirée.
4. **Adapte chaque item.** Remplace les mots génériques par ceux de ton jeu, suis la ligne « Adapter », écris le test, et vérifie qu'il échoue quand tu casses exprès la règle (sinon il ne teste rien). Dans le `CERTIFICATION.md` du jeu, note d'où vient l'item :

   ```markdown
   ### P-04 · Un vote arrivé après la fin de son duel ne compte pour aucun autre duel
   - **Origine :** VOTE-09
   - **Test :** `crates/core/src/tests/properties.rs`, `a_late_vote_does_not_land_on_the_next_duel`
   ```

5. **Justifie chaque item écarté en une ligne,** dans une section « Items du catalogue écartés » du `CERTIFICATION.md` du jeu. Exemple : « TXT-08 : pas de filtre de mots, on joue entre amis. » Un fichier entier peut s'écarter d'une ligne : « `physique.md` : rien ne bouge, c'est un quiz. »

| Ton jeu a… | Lis |
|---|---|
| des corps qui bougent, se touchent, tombent, rebondissent | [`physique.md`](physique.md) (`PHY`) |
| du temps réel : commandes tenues, sauts, réflexes, rythme, boucle à pas fixe | [`temps-reel.md`](temps-reel.md) (`TR`) |
| deux joueurs qui peuvent agir au même instant, ou finir à égalité | [`egalites.md`](egalites.md) (`EGA`) |
| des réponses, des votes, un buzzer, des rôles désignés par les autres | [`vote-quiz.md`](vote-quiz.md) (`VOTE`) |
| des tours, des étapes, une annulation | [`tour-par-tour.md`](tour-par-tour.md) (`TOUR`) |
| des points, de l'argent, des cartes, des objets, des tuiles | [`economie.md`](economie.md) (`ECO`) |
| des manches, une fin, un « on rejoue » | [`progression.md`](progression.md) (`PROG`) |
| du texte tapé que le jeu compare, coupe, filtre ou insère | [`texte.md`](texte.md) (`TXT`) |
| du hasard : paquets, rôles, premier joueur, apparitions | [`hasard.md`](hasard.md) (`HAS`) |
| des nombres, classements, minuteurs ou textes variables sur la TV | [`affichage.md`](affichage.md) (`AFF`) |

Presque tous les jeux de soirée cochent les trois dernières lignes et `progression.md`.

## Proposer un nouvel item

Un bug trouvé en jouant suit la règle du commun (`CERTIFICATION.md`, « La règle du bug trouvé en jouant ») : on le reproduit en test, on écrit dans `LEARNINGS.md` pourquoi aucun item ne l'a vu, et on ajoute un item au `CERTIFICATION.md` du jeu. Si la faille n'est pas propre à ce jeu (un autre jeu de soirée pourrait l'avoir), elle a sa place ici aussi.

- **L'agent du jeu la propose de lui-même,** sans attendre qu'on le lui demande, par une **pull request** sur le dépôt commun (pas une issue).
- **La PR ajoute l'item au bon fichier,** au format exact du catalogue, avec l'identifiant libre suivant de sa famille, une ligne dans l'index plus bas, et **un lien vers le test du jeu qui a trouvé la faille**. Le « bug d'origine » est alors ce bug-là, décrit en une à trois phrases.
- **C'est l'hôte qui décide** de la fusionner ou non : rien n'entre dans le catalogue sans son choix. En attendant, l'item reste dans le `CERTIFICATION.md` du jeu, qui le teste déjà.
- **Si l'agent ne peut pas ouvrir de PR lui-même,** il en écrit le contenu (le fichier visé et l'item complet) pour l'auteur du jeu, qui l'ouvrira.

Un item qui manque de famille peut ouvrir un nouveau fichier, avec un nouveau préfixe stable. Un identifiant publié ne change jamais et n'est jamais réutilisé, même si l'item est retiré : des jeux le citent dans leur `origine`.

## Les sources

Chaque « bug d'origine » renvoie à une source ouverte et lue au moment de l'écrire : une issue de jeu ou de moteur open source, un wiki, une spécification, un article. Certaines sont faibles (une issue réduite à son titre) : l'item le dit. Quand l'item illustre un principe plutôt qu'un bug, il le dit aussi. Les items tirés de nos propres jeux renvoient à leur `LEARNINGS.md` et au nom du test.

Les textes des sources ne sont pas recopiés : ils sont résumés. Une source qu'on n'a pas pu ouvrir n'entre pas dans le catalogue.

## Index

### [`physique.md`](physique.md)
- PHY-01 · Un corps rapide ne traverse jamais un mur fin sans le toucher
- PHY-02 · Marcher sur un sol plat fait de tuiles ne bloque jamais à une jointure
- PHY-03 · Aucune combinaison d'entrées ne dépasse le plafond de vitesse, dans un sens comme dans l'autre
- PHY-04 · Aucune position, vitesse ou force ne devient NaN ou infinie
- PHY-05 · Un pas de durée nulle ou une direction de longueur nulle ne corrompt rien
- PHY-06 · Aucun corps ne sort du monde ; s'il tombe dans un trou, il revient en jeu en un temps borné
- PHY-07 · Sans entrée, l'énergie totale n'augmente jamais
- PHY-08 · Un tas d'objets laissé seul finit par se poser, sans trembler ni s'enfoncer
- PHY-09 · Deux joueurs ne restent jamais coincés l'un dans l'autre
- PHY-10 · Un corps poussé contre un mur n'entre jamais dedans ni ne passe de l'autre côté
- PHY-11 · Un joueur posé sur une plateforme mobile ne passe pas au travers et ne flotte pas au-dessus
- PHY-12 · Une longue partie ou un réglage extrême ne fait jamais diverger la simulation

### [`temps-reel.md`](temps-reel.md)
- TR-01 · Le même temps, découpé autrement en images, donne exactement la même partie
- TR-02 · Après un gel de l'hôte, la boucle ne s'emballe pas pour rattraper le retard
- TR-03 · Un appui plus court qu'un tick n'est jamais perdu
- TR-04 · Un appui n'est jamais vu deux fois, et un bouton tenu ne se répète pas tout seul
- TR-05 · Un saut pressé juste après avoir quitté un rebord part quand même, et une seule fois
- TR-06 · Un saut pressé juste avant d'atterrir part à l'atterrissage, et une seule fois
- TR-07 · Tenir le saut plus longtemps ne donne jamais un saut plus bas
- TR-08 · Aller en diagonale n'est jamais plus rapide (ni plus lent) qu'en ligne droite
- TR-09 · Une valeur d'axe absurde ou un bruit au repos ne fait pas bouger le personnage
- TR-10 · Une pause gèle tout, et une nouvelle partie ne commence jamais en pause
- TR-11 · Une direction tenue cesse d'agir dès qu'on n'en a plus de nouvelles, et une vieille direction ne fait plus bouger
- TR-12 · Une fenêtre de réussite est symétrique, monotone, et ses bords sont fixés une fois pour toutes

### [`egalites.md`](egalites.md)
- EGA-01 · Le numéro d'un joueur ne décide jamais seul qui gagne une simultanéité
- EGA-02 · Des actions simultanées sont toutes évaluées sur l'état d'avant, puis appliquées ensemble
- EGA-03 · Le classement d'une course suit les temps, et deux arrivées identiques reçoivent la même chose
- EGA-04 · Des scores égaux ne sont jamais présentés comme la victoire de l'un
- EGA-05 · Le classement ne dépend que des scores et de la règle de départage
- EGA-06 · Deux scores égaux ont toujours le même rang affiché
- EGA-07 · Une égalité ne bloque jamais la phase qui doit la trancher
- EGA-08 · Quand un minuteur expire, le résultat tient compte de ce que l'autre pouvait encore faire, et le texte dit la même chose

### [`vote-quiz.md`](vote-quiz.md)
- VOTE-01 · Un joueur a au plus une réponse et un vote par question, et « tout le monde a répondu » compte des joueurs distincts
- VOTE-02 · Renvoyer le même vote ne le change pas
- VOTE-03 · Les points d'une manche ne sont comptés qu'une fois, et les totaux sont la somme des parts
- VOTE-04 · Le bulletin d'un joueur parti ne compte plus, et un revote repart d'une urne vide
- VOTE-05 · Si le joueur au cœur du vote part, la manche se termine quand même avec un résultat défini
- VOTE-06 · Un buzz simultané donne la main à exactement un joueur, qui ne se la fait pas reprendre
- VOTE-07 · Chaque consigne va au bon nombre de joueurs, et chacun en reçoit autant que les autres
- VOTE-08 · Un contenu écrit par les joueurs est joué exactement une fois
- VOTE-09 · Un vote arrivé en retard ne tombe jamais sur la question suivante, et personne ne vote pour sa propre réponse
- VOTE-10 · Aucun secret ne fuit dans la vue d'un joueur avant sa révélation
- VOTE-11 · Une question sans aucune réponse ni aucun vote se compte sans planter

### [`tour-par-tour.md`](tour-par-tour.md)
- TOUR-01 · Seul le joueur dont c'est le tour agit, annulation comprise
- TOUR-02 · Annuler un coup rend exactement l'état d'avant, jusqu'aux détails
- TOUR-03 · Une annulation ne fait jamais revenir un joueur parti
- TOUR-04 · Le tour passe toujours à un joueur présent, sinon la manche se termine
- TOUR-05 · Une fin de phase n'avance jamais le tour deux fois, et chaque joueur joue une fois par tour de table
- TOUR-06 · Deux causes de fin de tour dans le même pas finissent un seul tour, sans perdre la demande explicite
- TOUR-07 · Les conditions de fin se lisent sur l'état à jour, et une entrée ne déclenche qu'un nombre borné de transitions
- TOUR-08 · La partie peut finir à tout moment, même avant le premier tour, et le coup final compte
- TOUR-09 · Un compteur à seuil déclenche son effet exactement au seuil, ni avant, ni après

### [`economie.md`](economie.md)
- ECO-01 · Un objet n'est jamais perdu ni dupliqué : chacun est à exactement un endroit
- ECO-02 · Un échange est appliqué des deux côtés ou pas du tout
- ECO-03 · Un échange impossible est refusé sans rien faire disparaître, et livre exactement ce qui était offert
- ECO-04 · Une transition (fin de manche, sauvegarde, rechargement) ne crée ni ne perd rien
- ECO-05 · L'effet d'une action n'est jamais écrasé par une copie périmée de l'état
- ECO-06 · Faire puis défaire ne rapporte jamais rien
- ECO-07 · Une même chose ne rapporte qu'une fois
- ECO-08 · Aucune boucle de transferts entre joueurs ne crée de valeur
- ECO-09 · Aucun compteur ne déborde, ne revient à zéro ni ne change de signe
- ECO-10 · Partager ou arrondir ne crée ni ne détruit rien
- ECO-11 · Une commande répétée par un double appui n'est payée qu'une fois

### [`progression.md`](progression.md)
- PROG-01 · Aucun numéro de manche ou de niveau ne produit un état impossible
- PROG-02 · Une manche ne lit jamais ce qu'une autre a laissé
- PROG-03 · Après « rejouer », l'état est celui d'une partie neuve avec les mêmes joueurs
- PROG-04 · Quelle que soit la façon dont une partie finit, la suivante se joue avec les mêmes réglages
- PROG-05 · Aucune échéance ne se déclenche après la fin de sa partie ou de sa phase
- PROG-06 · Les effets de la fin s'appliquent une seule fois
- PROG-07 · Quand le dernier joueur attendu a joué, la phase passe tout de suite
- PROG-08 · Le jeu n'attend jamais un joueur qui ne peut pas agir

### [`texte.md`](texte.md)
- TXT-01 · Deux réponses identiques à l'écran ne sont jamais jugées différentes à cause de leur encodage Unicode
- TXT-02 · La clé de comparaison appliquée deux fois donne le même résultat qu'une fois
- TXT-03 · « Ces deux réponses sont la même » ne dépend jamais de la langue d'un appareil
- TXT-04 · Mettre un texte en majuscules ne lui fait pas dépasser son budget
- TXT-05 · Couper un texte ne coupe jamais au milieu d'un caractère ni d'un graphème
- TXT-06 · Deux prénoms qui se ressemblent à l'œil ne coexistent pas
- TXT-07 · Un caractère invisible ne permet jamais de prendre le nom d'un autre
- TXT-08 · Le filtre de mots ne censure jamais un mot innocent qui contient un mot interdit
- TXT-09 · Un texte de joueur inséré dans une phrase n'est jamais réinterprété comme un gabarit
- TXT-10 · Le contrôle des doublons porte sur le nom final, noms réservés compris

### [`hasard.md`](hasard.md)
- HAS-01 · Un mélange donne chaque ordre avec la même probabilité
- HAS-02 · Chaque élément peut finir à chaque place, y compris la dernière et la sienne
- HAS-03 · Un tirage borné ne favorise jamais les petites valeurs
- HAS-04 · Deux parties de la soirée ne reçoivent jamais la même graine par défaut
- HAS-05 · L'espace des graines n'est jamais réduit au point de rendre la partie devinable
- HAS-06 · Seules les actions de jeu acceptées consomment du hasard
- HAS-07 · Une question ne revient jamais avant que le paquet soit épuisé, ni deux fois de suite entre deux paquets
- HAS-08 · Mêler des éléments imposés et des éléments tirés ne crée jamais de doublon
- HAS-09 · Sur une partie, chacun est désigné à peu près autant que les autres

### [`affichage.md`](affichage.md)
- AFF-01 · Aucun nombre affiché n'a plus de chiffres que l'écran n'en prévoit
- AFF-02 · Le temps restant affiché ne devient jamais négatif, ne remonte jamais, et vaut 0 au changement de phase
- AFF-03 · Le rang est calculé sur la même valeur que celle qui est affichée
- AFF-04 · Les pourcentages de votes affichés font 100 et ne trompent pas
- AFF-05 · Un message qui contient un nombre est toujours accordé
- AFF-06 · Chaque écran de la TV a son pire cas, construit à partir des limites
- AFF-07 · Le contenu du jeu respecte les limites de la TV, une fois les prénoms insérés
