// The driver keeps direction through a finite deck, reversing only at an end.
// Its oracle reads semantic progression, never animated pixels or chat bubbles.
export class AdvanceCycle {
  constructor() { this.direction = "forward"; }
  choose(forward, backward) {
    if (typeof forward !== "boolean" || typeof backward !== "boolean") throw new Error("advance availability must be boolean");
    if (!(this.direction === "forward" ? forward : backward)) this.direction = this.direction === "forward" ? "backward" : "forward";
    if (!(this.direction === "forward" ? forward : backward)) throw new Error("no declared progression action is available");
    return this.direction;
  }
}

export async function advanceOnce(tab, definition, cycle, press) {
  if (typeof definition.progress !== "string" || !definition.progress.trim()) throw new Error("advance.progress must identify semantic progression");
  const observed = await tab.page.evaluate(({progress,can_forward,can_backward,cycle}) => {
    const value = (0,eval)(progress);
    if (value === undefined || value === null) throw new Error("semantic progression is absent");
    return { before:JSON.stringify(value), forward:cycle ? (0,eval)(can_forward) : true, backward:cycle ? (0,eval)(can_backward) : false };
  }, {...definition,cycle:Boolean(definition.forward)});
  const direction = definition.forward ? cycle.choose(observed.forward,observed.backward) : "forward";
  const action = definition.forward ? definition[direction] : definition;
  await press(tab,action);
  const changed=await tab.page.waitForFunction(({expression,before}) => {
    const value=(0,eval)(expression);
    if(value===undefined||value===null)throw new Error("semantic progression disappeared");
    const token=JSON.stringify(value);
    return token!==before ? token : false;
  },{expression:definition.progress,before:observed.before},{timeout:3000,polling:"raf"});
  try { return {direction,before:observed.before,after:await changed.jsonValue()}; }
  finally {await changed.dispose();}
}

/** Drives a running game to a declared screen, one organizer step at a time,
 * looking before each step: the step after that screen would leave it. A
 * screen it never reaches fails here, not as a measurement of another one. */
export async function advanceUntil(reached, step, { maxSteps }) {
  for (let steps = 0; ; steps++) {
    if (await reached()) return steps;
    if (!step) throw new Error("écran attendu absent, et aucun moyen d'avancer déclaré (`advance`)");
    if (steps === maxSteps) throw new Error(`écran attendu absent après ${maxSteps} étapes`);
    await step();
  }
}
