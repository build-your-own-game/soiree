# {{nom}} : ce qu’on a appris

Un test navigateur a trouvé qu’un remplacement complet du DOM perdait un appui
en cours dès qu’un autre joueur faisait changer les compteurs. Le bouton reste
désormais attaché ; seules les vues passives sont remplacées. Le test
`tools/press-check.mjs` tient un appui pendant l’action d’un autre joueur et
vérifie identité du bouton et total exact.

Le jeu de départ reprend la séparation simulation/hôte/page de la démo salon.
Les garanties de transport et leurs sources restent dans `.soiree/salon/README.md`.
Un changement de mécanique ajoute ses références et ses tests ici.

## Écarts au manifeste

1. **Respecté.** Les compteurs ne débordent pas ; des propriétés vérifient les entrées et les replays. Les mutations de next_score sont réellement détectées par les tests du modèle dans l’outil.
2. **Écart.** Raison : un compteur événementiel ne simule aucun mouvement entre deux entrées ; aucun pas fixe ni interpolation ne sont nécessaires.
3. **Respecté.** Les entrées ordonnées et horodatées sont journalisées à la frontière de simulation. Le replay compare tout l’état, et un journal fictif versionné sert de test. Le format décide de la lecture ; le numéro de sources informe le diagnostic sans rendre les fixtures caduques à chaque commentaire. Le jeu n’utilise ni hasard ni contenu externe : la graine du journal est zéro.
4. **Respecté.** Le cœur compile sans navigateur ni GPU ; l’hôte et les vrais clients salon sont exercés sur 2048 réseaux à graine, avec coupures, retours et duplication.
5. **Écart.** Raison : le compteur se dessine en DOM et n’a aucun calcul navigateur à paralléliser. Salon fournit COOP/COEP ; WebGPU et les threads n’amélioreraient pas cette mécanique.
6. **Respecté.** Salon possède les transports, les identités et la reconnexion ; le jeu ne connaît que les appareils et leurs entrées.
7. **Respecté.** Le harnais mesure la cadence, l’arrivée et le délai du bouton téléphone au compteur TV, avec des téléphones Chromium et WebKit. Les rapports sont propres à chaque passage ; les mesures physiques restent à refaire sur le matériel de la soirée.
8. **Respecté.** Messages et journaux utilisent leurs types Rust partagés ; les noms du DOM sont générés depuis HTML et CSS. La version salon est calculée depuis les sources.
