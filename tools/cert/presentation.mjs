/** Observe the specified refusal in the actual visible page. A disconnected
 * page, a hidden message, or arbitrary nonempty text is not a refusal oracle. */
export async function refusalMessage(page, expected) {
  if (!expected || typeof expected.selector !== 'string' || !expected.selector.trim()
      || typeof expected.text !== 'string' || !expected.text.trim()) {
    throw new Error('cert.json.refused doit nommer le sélecteur et le texte du refus de salle pleine');
  }
  const message = page.locator(expected.selector).filter({ hasText: expected.text }).first();
  await message.waitFor({ state: 'visible', timeout: 3000 });
  return message.evaluate((element, text) => {
    const bounds = element.getBoundingClientRect();
    const normalize = value => value.replace(/\s+/g, ' ').trim();
    const actual = normalize(element.textContent ?? '');
    if (!actual.includes(normalize(text))) throw new Error('le texte du refus a changé');
    for (let parent = element; parent; parent = parent.parentElement) {
      const style = getComputedStyle(parent);
      if (style.display === 'none' || style.visibility !== 'visible' || Number(style.opacity) === 0
          || parent.hidden || parent.getAttribute('aria-hidden') === 'true') {
        throw new Error('le refus est masqué');
      }
    }
    if (bounds.width <= 0 || bounds.height <= 0 || bounds.right <= 0 || bounds.bottom <= 0
        || bounds.left >= innerWidth || bounds.top >= innerHeight) throw new Error('le refus est hors écran');
    return { text: actual, bounds: { x: bounds.x, y: bounds.y, width: bounds.width, height: bounds.height } };
  }, expected.text);
}

/** Serializable browser observer, sampled both while joining and in play. */
export function phoneFieldSizes() {
  return [...document.querySelectorAll('input, textarea, select, [contenteditable="true"], [contenteditable="plaintext-only"]')]
    .filter(element => {
      const rect = element.getBoundingClientRect();
      return rect.width > 0 && rect.height > 0 && getComputedStyle(element).visibility === 'visible';
    })
    .map(element => ({ tag: element.tagName.toLowerCase(), id: element.id,
      font_px: Number.parseFloat(getComputedStyle(element).fontSize) }));
}

export function fieldSizeFailures(samples) {
  return samples.flatMap(sample => sample.fields
    .filter(field => !Number.isFinite(field.font_px) || field.font_px < 16)
    .map(field => `${sample.label} ${field.tag}${field.id ? `#${field.id}` : ''} : ${field.font_px} px`));
}

/** Whether a page came back to the seat it had: the same token, and the same
 * device in a Welcome that arrived after the reload. "unknown" is not a pass. */
export function seatAfterReturn(before, after) {
  if (before.token === null || !Number.isInteger(before.device)) return "unknown";
  if (after.token === null || !Number.isInteger(after.device) || !after.welcomed) return "unknown";
  return after.token === before.token && after.device === before.device ? "same" : "other";
}
