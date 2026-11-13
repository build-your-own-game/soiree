// Les réglages d'une manette, appliqués à la page quand son document est prêt
// (`atDocumentEnd`) : pas de zoom, pas de sélection de texte, pas de menu au
// toucher long. Rien ici ne fait partie du contrat avec la page.
(function () {
  "use strict";

  // Pas de zoom au pincement : on complète la balise viewport de la page.
  // Une page sans balise viewport est laissée telle quelle.
  var viewport = document.querySelector('meta[name="viewport"]');
  if (viewport && !/user-scalable/.test(viewport.content)) {
    viewport.content += ", maximum-scale=1, user-scalable=no";
  }

  // Pas de sélection ni de menu contextuel, sauf dans les champs de saisie
  // (un joueur doit pouvoir taper son nom). `touch-action: manipulation`
  // retire le zoom au double toucher.
  var style = document.createElement("style");
  style.textContent =
    "html { -webkit-user-select: none; user-select: none; -webkit-touch-callout: none;" +
    " -webkit-text-size-adjust: 100%; touch-action: manipulation; }" +
    " input, textarea, [contenteditable] { -webkit-user-select: text; user-select: text; }";
  (document.head || document.documentElement).appendChild(style);
})();
