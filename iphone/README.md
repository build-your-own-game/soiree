# Soirée pour iPhone

Une manette pour tous les jeux de la soirée. L'app scanne le QR code affiché sur la télé, ouvre la page « télécommande » du jeu, et lui donne ce que Safari lui refuse : les capteurs de mouvement (Safari les interdit hors HTTPS) et les vibrations (Safari ne sait pas vibrer).

L'app ne connaît aucun jeu. Elle complète les API web standard : une page écrite pour le web marche ici sans rien savoir de l'app.

## Le contrat avec la page

L'app injecte `Soiree/pont.js` dans le cadre principal, avant tout script de la page.

### `window.soiree`

```js
window.soiree = { native: true, version: 1, haptic(kind, intensity) }
```

- `kind` : `"impact"`, `"success"`, `"failure"` ou `"tick"`. Un genre inconnu vibre comme `"impact"`.
- `intensity` : de 0 à 1, bornée, 1 par défaut. 0 ne vibre pas.
- Hors de l'app, `window.soiree` n'existe pas : `if (window.soiree?.native)`.

### La permission des capteurs

`DeviceMotionEvent.requestPermission()` et `DeviceOrientationEvent.requestPermission()` répondent toujours `"granted"`. Les deux interfaces existent, même en HTTP.

### Les événements `devicemotion`

Envoyés sur `window` à 100 Hz depuis CoreMotion, tant qu'une page est chargée et que l'app est au premier plan. Ce sont des `Event` ordinaires qui portent les champs standard :

| Champ | Unité | Contenu |
|---|---|---|
| `acceleration {x, y, z}` | m/s² | sans la gravité |
| `accelerationIncludingGravity {x, y, z}` | m/s² | avec la gravité |
| `rotationRate {alpha, beta, gamma}` | deg/s | `alpha` autour de x, `beta` autour de y, `gamma` autour de z, comme Safari iOS |
| `interval` | ms | 10 |

Les axes, téléphone en portrait face à soi : x vers la droite, y vers le haut, z sort de l'écran.

Les signes :

- **Accélérations : ceux de Safari sur iOS**, par le même calcul que WebKit (`userAcceleration × 9,80665`, et `(userAcceleration + gravity) × 9,80665`). Téléphone à plat, écran vers le haut : `accelerationIncludingGravity.z` vaut −9,81. Android et la norme W3C donnent +9,81.
- **Rotations : règle de la main droite** autour de chaque axe, comme Safari.

Le nom des axes de rotation suit Safari iOS, pas la norme W3C (qui dit `alpha` autour de z, `beta` autour de x, `gamma` autour de y) : une page se comporte ainsi de la même façon dans l'app et dans Safari sur iPhone. Le détail est dans `Soiree/Mouvement.swift`.

L'app n'envoie pas d'événements `deviceorientation`.

### `navigator.vibrate(durée | motif)`

Remplacé par une vibration native, renvoie `true`. Le motif est celui du web, en ms : `[vibre, pause, vibre, …]`, dix secondes au plus. `vibrate(0)` arrête la vibration en cours.

### Ce que l'app règle d'elle-même

Pas de zoom, pas de rebond de défilement, pas de sélection de texte (sauf dans les champs de saisie), pas de menu au toucher long, le son sans geste préalable et même en mode silencieux, l'écran toujours allumé, le portrait verrouillé. La page s'étend sous l'encoche : à elle de respecter les zones sûres (`viewport-fit=cover` et `env(safe-area-inset-*)`). Un petit bouton en haut à droite permet de quitter.

### Ouvrir l'app depuis Safari

```
soiree://ouvrir?u=<adresse encodée>
```

Par exemple `soiree://ouvrir?u=http%3A%2F%2F192.168.1.20%3A8080%2Ft`. L'app n'ouvre que des adresses `http` ou `https`. En `http`, seulement celles du réseau local (une adresse IP ou un nom en `.local`).

## L'installer sur un iPhone

Il faut un Mac avec Xcode, et un iPhone sous iOS 17 ou plus.

1. Ouvre `Soiree.xcodeproj` dans Xcode.
2. Dans la cible **Soiree**, onglet **Signing & Capabilities**, choisis ton équipe (un identifiant Apple gratuit suffit). Si l'identifiant `game.buildyourown.soiree` est refusé, change-le.
3. Branche l'iPhone, choisis-le comme destination, lance (⌘R).
4. La première fois, sur l'iPhone : **Réglages > Général > VPN et gestion de l'appareil**, fais confiance à ton certificat. Xcode peut aussi demander d'activer le mode développeur.

Avec un identifiant Apple gratuit, l'app cesse de s'ouvrir au bout de sept jours : relance-la depuis Xcode.

Au premier jeu, l'iPhone demande l'accès à l'appareil photo, puis au réseau local. Il faut dire oui aux deux.

## L'essayer

Sur un iPhone : lance un jeu sur l'ordinateur, scanne le QR code de la télé avec l'app. Sans QR code, tape l'adresse écrite dessous.

Dans le simulateur, sans iPhone :

```sh
xcodebuild -project Soiree.xcodeproj -scheme Soiree \
  -destination 'generic/platform=iOS Simulator' -derivedDataPath /tmp/soiree-iphone \
  CODE_SIGNING_ALLOWED=NO build
xcrun simctl boot 'iPhone 17 Pro'
xcrun simctl install booted /tmp/soiree-iphone/Build/Products/Debug-iphonesimulator/Soiree.app
xcrun simctl launch booted game.buildyourown.soiree
xcrun simctl openurl booted 'soiree://ouvrir?u=http%3A%2F%2F192.168.1.20%3A8080%2Ft'
```

Le simulateur n'a ni appareil photo, ni capteurs, ni vibrations. L'accueil y reste utilisable par le champ d'adresse, et l'app y envoie des événements `devicemotion` d'un téléphone immobile tenu droit, pour qu'une page voie le flux arriver.
