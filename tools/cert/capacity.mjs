/** A smaller room cannot seat the ordinary count: its players are all there is. */
export function ordinaryPhones(requested, maximum) {
  return Number.isSafeInteger(maximum) && maximum >= 1 ? Math.min(requested, maximum) : requested;
}

/** A global gesture total cannot stand in for participation by every phone. */
export function capacityActivityFailures(participants, maximum) {
  const failures = [];
  if (participants.length !== maximum) failures.push(`${participants.length} participants mesurés pour ${maximum} places`);
  const ids = new Set();
  for (const row of participants) {
    if (ids.has(row.index)) failures.push(`participant dupliqué : ${row.index}`);
    ids.add(row.index);
    if (row.joined !== true || row.live !== true) failures.push(`${row.label} ne garde pas sa place et une connexion vivante`);
    if (!Number.isSafeInteger(row.gestures) || row.gestures <= 0) failures.push(`${row.label} n'a terminé aucun geste natif`);
  }
  return failures;
}
