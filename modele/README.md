# Mon jeu

Un jeu pour nos soirées : il tourne sur l'ordinateur de l'hôte, s'affiche sur la télé, et chacun joue avec son téléphone.

## Lancer

```sh
cargo run --release
```

Le terminal affiche l'adresse de l'écran, à ouvrir sur l'ordinateur, et celle que les téléphones ouvrent. L'écran montre aussi un QR code.

## Vérifier

```sh
soiree check
```

Un jeu qui ne passe pas cette vérification n'est pas joué à la soirée.
