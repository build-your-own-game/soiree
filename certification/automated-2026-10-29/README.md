# Certification automatisée — investigation du 29 octobre 2026

Chantier en cours : aucune campagne complète verte n'est encore attestée.
Les échecs précédents restent dans `../robustesse-2026-10-29/` et les dossiers
`games/*/certification/endurance-2026-10-29/` ; ils ne sont pas remplacés par
les reproductions ou réussites ciblées ci-dessous.

## Causes reproduites et corrections

- **WebRTC.** Une réponse SDP saine retardée de quatre secondes arrive après
  l'abandon client à trois secondes, avant tout datagramme. La même négociation
  ouvre son canal si elle peut finir. La fenêtre absolue est alignée sur celle
  de l'hôte, quinze secondes ; les erreurs natives restent immédiatement
  terminales. Les régressions ont aussi révélé une seconde horloge concurrente
  de cinq secondes dans `salon-core`, susceptible de relancer RTC indéfiniment.
  Son articulation avec le budget de négociation est corrigée et éprouvée.
  Ces mécanismes ne sont pas une attribution rétrospective certaine de 7222.
- **Tier List.** L'encodage PNG synchrone bloque réellement le rendu. Le
  contrôle à bitmap identique sans cet encodage évite le dépassement. Le
  canvas d'export devient CPU, le dessin est coopératif et le Blob asynchrone ;
  ses URL appartiennent au dialogue. Les pixels et les durées sont testés.
- **Mesure P1.** Suspendre Node pendant que le navigateur termine sa mesure
  produisait un faux timeout. Un reçu corrélé et horodaté dans la page permet
  de récupérer uniquement cette mesure déjà achevée dans son délai ; aucun
  nouvel échantillon n'est lancé et un vrai arrêt du navigateur échoue encore.
- **Toxiproxy.** Le Dial TCP sans échéance de l'outil officiel bloque sa boucle
  d'acceptation ; sa commande stop attend cette boucle. Le backlog saturé le
  reproduit alors que `/version` répond toujours. Un connecteur possédé,
  borné et annulable rejoint la vraie adresse LAN ; les fautes restent celles
  du moteur officiel. Voir `../robustesse-2026-10-29/toxiproxy-dial/`.

## Couverture du passage complet

Le scénario `capacity` éprouve la capacité déclarée avec de vrais téléphones,
des actions causales et une mesure de rendu, puis repart sur des données
propres pour les autres scénarios. Le nombre de téléphones ordinaires ne réduit
plus silencieusement le périmètre de R3. Les commandes UI/texte ont chacune
un hôte propre ; la salle pleine des autres parcours ne consomme pas leurs
places et leurs états ne se contaminent pas.

Le nouveau scénario `text` remplace le contrôle T2 de monkey, qui ne produisait
aucun succès dédié. Il exerce un corpus déterministe, son effet ou son refus,
sa literalité et ses vrais cadres, avec des contrôles négatifs. Le refus de
salle pleine C8 doit montrer un message déclaré et visible. R1 ajoute une
action causale sous latence ; le ressenti physique reste une revue au salon.
T4 mesure les champs réellement présents à l'arrivée et en partie.

Le rôle `display.desktop` ne s'applique qu'à un éditeur solo explicitement
motivé, sans téléphone, socket ou commandes de phase. Il distingue le domaine
des cadres défilants T2 de la zone sûre TV T3. Un simple `layout:false`, une
faute de rôle ou un jeu avec téléphones ne peut pas obtenir cette exclusion.
Sidequest garde T3 TV et vérifie les cadres effectivement soumis au GPU.

Les budgets de cadence, latence d'entrée, mémoire et capacité ne sont pas
relevés. Les observations sur iPhone/TV/Wi-Fi physiques restent séparées.
Les reçus des passages complets figés seront ajoutés ici après leur exécution.
