# Certification du modèle v0.2.0

Passage complet du 28 octobre 2026, graine 6839, sur un jeu créé par le
binaire candidat avec `soiree new`. Les trois dépendances salon du jeu de
test pointaient vers le checkout commun candidat, avant publication de son
étiquette. Le rapport confirme que ni les sources du jeu ni le harnais
n’ont changé pendant le passage. Les trois jeux du dossier de l’hôte ont
également terminé leur passage complet sur le même candidat.

Le premier passage du modèle s’était arrêté sur un geste natif WebKit
bloqué ; son rapport est conservé localement. Le diagnostic a reproduit
séparément un swipe produisant deux activations et un accusé de hover
retenu jusqu’à un rendu. Le harnais exerce maintenant un seul drag et
n’attend plus cet accusé avant d’envoyer l’appui qui le libère. Les
contre-épreuves natives de ces deux correctifs sont exécutées par les tests
du harnais. Elles n’établissent pas rétroactivement la cause exacte de
tous les anciens blocages du pilote.

Le passage final utilise les listeners réentrants `Fn`. Leur ancien
trampoline `FnMut` provoquait une erreur sur les clics imbriqués du
sélecteur de fichiers de Tier-list ; une contre-épreuve dans les deux
moteurs et le parcours réel du jeu la reproduisent.

Ce passage ne couvre ni l’endurance de trente minutes ni les vrais claviers
et appareils du salon. Le rapport distingue les catégories UI non
applicables de celles réellement exécutées.
