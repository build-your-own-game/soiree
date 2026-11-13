# Rejoindre la soirée

> Tu veux faire un jeu pour la soirée ? Copie tout ce texte et colle-le dans ton assistant IA (ChatGPT, Claude, Gemini, ou un agent de code). Il te guidera.

Bonjour. Des amis et moi faisons chacun un jeu de soirée avec un assistant IA, puis on y joue tous ensemble chez l'un de nous : son ordinateur est branché à la télé, et chacun joue avec son téléphone. Je veux faire le mien, et je veux que tu m'y aides.

Je n'ai peut-être jamais codé. Parle-moi simplement, une étape à la fois, et vérifie que chaque étape a marché avant de passer à la suivante.

## D'abord, dis-moi ce que tu peux faire

- **Tu peux lancer des commandes et écrire des fichiers sur mon ordinateur** (tu es un agent de code : OpenCode, Claude Code, Codex, Cursor…). Fais alors les étapes ci-dessous toi-même, et ne me demande que ce que tu ne peux pas faire à ma place.
- **Tu es une page de discussion** (ChatGPT, Claude ou Gemini, dans un navigateur ou une app). Dis-le-moi tout de suite et clairement : tu ne peux pas faire ce jeu à ma place, parce qu'il doit être compilé et essayé sur mon ordinateur. Ton rôle est de m'aider à installer un agent de code, puis de me dire de lui coller ce même texte. Si je n'en ai pas, propose-moi OpenCode (https://opencode.ai) : il s'installe en quelques minutes et propose des modèles gratuits pour commencer. Guide-moi pour mon système (Mac, Windows ou Linux), y compris pour ouvrir un terminal si je ne sais pas ce que c'est.

## Les étapes

1. **Un compte GitHub.** Si je n'en ai pas, guide-moi pour le créer sur https://github.com/signup. Dès que j'ai mon pseudo, dis-moi de l'envoyer à l'hôte de la soirée : il doit m'inviter dans l'organisation où vivent les jeux, et cela peut prendre un moment. On continue sans attendre sa réponse.
2. **Rust**, avec rustup : https://rustup.rs.
3. **L'outil de la soirée.** La première installation prend quelques minutes.
   ```sh
   cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.5.0 soiree
   ```
4. **Mon jeu.** Demande-moi un nom, en minuscules, chiffres et tirets. Puis, dans le dossier où je range mes projets :
   ```sh
   soiree new <nom>
   ```
5. **La suite est dans l'outil.** `soiree help` donne les commandes, `soiree manifeste` donne les règles de la soirée, et le fichier `AGENTS.md` du jeu dit comment travailler avec moi. Lis-les, lance le jeu pour me le montrer, puis demande-moi quel jeu je veux faire.
