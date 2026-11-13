// Le pont entre la page d'un jeu et l'app Soirée.
//
// L'app injecte ce script dans le cadre principal, avant tout script de la page
// (`atDocumentStart`). Il complète les API web standard pour que la page d'un
// jeu marche ici comme elle marcherait dans un navigateur qui a des capteurs
// de mouvement et des vibrations :
//
//   - `window.soiree`                         : dit à la page qu'elle est dans l'app, et fait vibrer ;
//   - `DeviceMotionEvent.requestPermission()` : répond toujours "granted" ;
//   - les événements `devicemotion`           : envoyés par l'app depuis CoreMotion, à 100 Hz ;
//   - `navigator.vibrate()`                   : remplacé par une vibration native.
//
// Le contrat est décrit dans `iphone/README.md`. Les pages des jeux en
// dépendent : ne change ni un nom ni une unité sans changer la version.
(function () {
  "use strict";

  if (window.soiree && window.soiree.native) {
    return; // Déjà installé dans ce document.
  }

  // --- Vers le natif -------------------------------------------------------

  // Envoie un message à l'app. Ne lève jamais d'erreur : une vibration qui
  // échoue ne doit pas casser un jeu.
  function envoyer(message) {
    try {
      window.webkit.messageHandlers.soiree.postMessage(message);
    } catch (erreur) {
      // Hors de l'app, ou pont coupé : on ignore.
    }
  }

  function borner(valeur, defaut) {
    var nombre = Number(valeur);
    if (valeur === undefined || valeur === null || !isFinite(nombre)) {
      return defaut;
    }
    return Math.min(1, Math.max(0, nombre));
  }

  // --- window.soiree -------------------------------------------------------

  var soiree = {
    native: true,
    version: 1,
    // kind : "impact", "success", "failure" ou "tick".
    // intensity : de 0 à 1, bornée, 1 par défaut.
    haptic: function (kind, intensity) {
      envoyer({ type: "haptic", kind: String(kind), intensity: borner(intensity, 1) });
    },
  };
  Object.freeze(soiree);
  Object.defineProperty(window, "soiree", { value: soiree, writable: false, configurable: false, enumerable: true });

  // --- La permission des capteurs ------------------------------------------

  // Dans Safari, `requestPermission` n'existe qu'en HTTPS et ouvre une
  // demande. Ici c'est l'app qui lit les capteurs : la réponse est toujours oui.
  function accorder() {
    return Promise.resolve("granted");
  }

  function installerPermission(nom) {
    var interfaceWeb = window[nom];
    if (typeof interfaceWeb !== "function") {
      // L'interface n'existe pas dans ce contexte (page en HTTP) : on en
      // définit une, pour que `DeviceMotionEvent.requestPermission` existe.
      interfaceWeb = function () {
        throw new TypeError("Illegal constructor");
      };
      interfaceWeb.prototype = Object.create(Event.prototype);
      Object.defineProperty(interfaceWeb, "name", { value: nom });
      Object.defineProperty(window, nom, { value: interfaceWeb, writable: true, configurable: true });
    }
    Object.defineProperty(interfaceWeb, "requestPermission", { value: accorder, writable: true, configurable: true });
  }

  installerPermission("DeviceMotionEvent");
  installerPermission("DeviceOrientationEvent");

  // --- Les événements devicemotion -----------------------------------------

  // L'app appelle cette fonction pour chaque échantillon de CoreMotion, avec
  // des valeurs déjà converties (voir `Mouvement.swift` pour les unités et les
  // signes) :
  //   ax, ay, az : accélération sans la gravité, en m/s² ;
  //   gx, gy, gz : accélération avec la gravité, en m/s² ;
  //   alpha, beta, gamma : vitesse de rotation, en deg/s ;
  //   intervalle : temps entre deux échantillons, en ms.
  // La page lit les champs par leur nom : un `Event` ordinaire, avec les
  // champs standard posés dessus, suffit.
  function mouvement(ax, ay, az, gx, gy, gz, alpha, beta, gamma, intervalle) {
    var evenement = new Event("devicemotion");
    evenement.acceleration = { x: ax, y: ay, z: az };
    evenement.accelerationIncludingGravity = { x: gx, y: gy, z: gz };
    evenement.rotationRate = { alpha: alpha, beta: beta, gamma: gamma };
    evenement.interval = intervalle;
    window.dispatchEvent(evenement);
  }
  // Un nom que la page ne peut pas remplacer : l'app l'appelle 100 fois par seconde.
  Object.defineProperty(window, "__soireeMouvement", { value: mouvement, writable: false, configurable: false, enumerable: false });

  // --- navigator.vibrate ---------------------------------------------------

  // Safari sur iPhone ne sait pas vibrer. Même signature que l'API standard :
  // une durée en ms, ou un motif [vibre, pause, vibre, …]. Renvoie `true`.
  // `vibrate(0)` ou `vibrate([])` arrête la vibration en cours.
  function vibrer(motif) {
    var liste = Array.isArray(motif) ? motif : [motif];
    var durees = [];
    for (var i = 0; i < liste.length && i < 64; i += 1) {
      var duree = Number(liste[i]);
      durees.push(isFinite(duree) && duree > 0 ? duree : 0);
    }
    envoyer({ type: "vibrate", pattern: durees });
    return true;
  }
  try {
    Object.defineProperty(Navigator.prototype, "vibrate", { value: vibrer, writable: true, configurable: true });
  } catch (erreur) {
    navigator.vibrate = vibrer;
  }
})();
