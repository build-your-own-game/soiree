/** Only applicable, selected chaos can establish an endurance observation. */
export function recurringScenarios(selected, { phones, socket }) {
  return ['cut', 'freeze', 'reload', 'hostile', 'monkey'].filter(name => selected.includes(name)
    && (name === 'monkey' || (name === 'hostile' ? socket : phones)));
}

/** Samples carry monotonic elapsed time separately from archival wall time. */
export function serverEndurance(samples) {
  if (!samples.every(sample => Number.isFinite(sample.elapsed_ms) && sample.elapsed_ms >= 0
      && Number.isFinite(sample.rss) && sample.rss > 0
      && Number.isSafeInteger(sample.threads) && sample.threads > 0)) {
    throw new Error('mesure serveur manquante ou invalide');
  }
  const warm = samples.find(sample => sample.elapsed_ms >= 300000);
  const last = samples.at(-1);
  if (!warm || !last || last.elapsed_ms <= warm.elapsed_ms) throw new Error('mesures serveur absentes après cinq minutes de chauffe');
  const perHourMiB = ((last.rss - warm.rss) / 1024) / ((last.elapsed_ms - warm.elapsed_ms) / 3600000);
  const eveningMiB = Math.max(0, perHourMiB * 6);
  const failures = [];
  if (eveningMiB > 256) failures.push(`pente de ${eveningMiB.toFixed(0)} Mio sur six heures > 256 Mio`);
  if (last.threads > warm.threads) failures.push(`threads ${warm.threads} → ${last.threads}`);
  return { warm, last, evening_mib: eveningMiB, failures };
}
