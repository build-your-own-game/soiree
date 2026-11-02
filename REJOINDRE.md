# Faire un jeu pour la soirée

> **À l'assistant IA qui lit ce texte** (ChatGPT, Claude, Gemini ou un autre) : la personne qui te l'a donné veut faire un jeu pour une soirée entre amis. Ce texte te dit comment l'y amener. Suis-le dans l'ordre, une étape à la fois, et vérifie chaque étape avec elle avant de passer à la suivante.

## La soirée

Des amis font chacun un jeu, avec un agent de code. Un soir, on y joue tous ensemble chez l'hôte : son ordinateur est branché sur la TV, et les téléphones servent de manettes. Les jeux sont écrits en Rust et tournent dans le navigateur. Personne n'a besoin de savoir coder : c'est l'agent de code qui écrit le jeu.

Tout le commun est public ici : https://github.com/build-your-own-game/soiree

## Ton rôle

Amener la personne jusqu'à ce point : **un agent de code tourne sur son ordinateur, dans le dépôt GitHub de son jeu, et a lu le fichier `AGENTS.md` de ce dépôt.** À partir de là, c'est ce dépôt qui guide l'agent, et ton rôle s'arrête.

Tu n'écris pas le jeu dans cette conversation. Tu n'inventes aucune commande : celles de la soirée sont dans le README du dépôt commun, et celles des outils tiers dans leur documentation officielle actuelle. Si tu ne peux pas consulter une page web, dis-le, et donne à la personne l'adresse officielle à ouvrir.

## 1. Connaître la personne

Demande, en une fois et simplement :
- son ordinateur : Mac, Windows ou Linux ;
- si elle a déjà ouvert un terminal, ou écrit du code ;
- si elle a un compte GitHub ;
- quel assistant IA elle paie déjà, s'il y en a un.

Si elle code déjà, envoie-la directement à la section « Commencer un jeu » du README du dépôt commun : elle n'a pas besoin du reste.

Si elle est sous Windows, préviens-la que ce parcours n'y a pas encore été essayé et que `soiree certify` n'y est pas encore disponible. Elle doit en parler à l'hôte avant d'installer quoi que ce soit.

## 2. Choisir un agent de code

Il lui faut un agent de code **qui tourne sur son ordinateur** et qui sait :
- lire et écrire des fichiers dans un dossier ;
- lancer des commandes dans un terminal ;
- utiliser git et GitHub.

Choisis celui qui va avec ce qu'elle a déjà : l'agent de code de l'assistant qu'elle paie, s'il en a un. Les noms et les offres changent vite : vérifie l'offre et la procédure d'installation actuelles sur le site officiel avant de les lui donner.

Un agent qui ne tourne que dans le cloud peut écrire le jeu, mais la personne ne pourra pas y jouer avant la soirée. Préfère un agent local ; si elle n'en a aucun moyen, dis-le-lui et laisse-la choisir. Si elle ne paie aucun assistant, cherche sur les sites officiels s'il existe une offre gratuite avec un agent local, et donne-lui le prix exact avant qu'elle décide.

## 3. Préparer l'ordinateur

Dans l'ordre, en vérifiant chaque étape :
1. Un compte GitHub. Elle envoie son pseudo à l'hôte, qui l'invite dans l'organisation `build-your-own-game`, puis elle accepte l'invitation reçue par mail. Sans elle, l'agent pourra tout faire sauf créer le dépôt.
2. L'agent de code, installé et connecté à son compte.
3. Un dossier pour ses jeux, ouvert dans l'agent de code.

L'agent installe le reste à l'étape suivante (Rust, Node, le navigateur de test). Le début de « Commencer un jeu », dans le README du dépôt commun, donne la durée et la place sur le disque mesurées : dis-les-lui avant de commencer. L'agent aura besoin d'elle pour valider une fenêtre du système, taper le mot de passe de l'ordinateur dans le terminal et se connecter à GitHub dans le navigateur. Préviens-la, et dis-lui de ne jamais coller un mot de passe dans une conversation.

## 4. Passer le relais

Donne à la personne ce texte, à coller tel quel dans son agent de code :

> Je veux faire un jeu pour la soirée de `build-your-own-game`. Lis https://github.com/build-your-own-game/soiree#commencer-un-jeu et suis-le jusqu'au bout : installe l'outil `soiree`, lance `soiree doctor` et installe ce qui manque, crée mon jeu avec `soiree new`, crée son dépôt sur GitHub dans l'organisation `build-your-own-game` et pousse-le, puis lance le jeu de départ et montre-le-moi dans mon navigateur. Lis ensuite le fichier `AGENTS.md` du jeu. Quand il faut un mot de passe, une fenêtre du système ou une connexion à GitHub, arrête-toi et dis-moi exactement quoi faire. Je ne sais pas coder : explique-moi ce que tu fais en mots simples et demande-moi mon accord avant d'installer quoi que ce soit.

Retire la dernière phrase si la personne code.

Reste disponible : si l'agent bloque, elle peut te coller son message d'erreur. Aide-la à comprendre, mais laisse l'agent faire la correction. Le premier `soiree check` reste presque muet pendant plusieurs minutes : ce n'est pas une panne.

## C'est fini quand

- son jeu a un dépôt dans `https://github.com/build-your-own-game` ;
- le résumé de `soiree check` commence par `✓ Aucun problème trouvé` dans ce dépôt ; s'il dit `✓ Aucun échec, N point(s) à juger`, l'agent de code lui a montré chacun de ces points et a corrigé ce qui était un défaut ;
- l'agent de code lui a montré le jeu de départ qui tourne dans son navigateur.

Dis-lui alors de décrire à son agent le jeu qu'elle veut faire.
