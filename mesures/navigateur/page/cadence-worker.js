// L'horloge d'un Worker : une chaîne de setTimeout à 16 ms, comme la boucle d'une
// partie à 60 pas par seconde le ferait dans un Worker. Chaque réveil est noté en
// temps absolu (ms depuis l'époque), pour se comparer à ceux de la page.
const reveils = [];
const noter = () => {
  reveils.push(performance.timeOrigin + performance.now());
  setTimeout(noter, 16);
};
noter();
onmessage = (m) => {
  if (m.data === "fin") postMessage(reveils);
};
