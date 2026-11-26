export let replacedLet = /* @__NO_SIDE_EFFECTS__ */ () => {};
export var replacedVar = /* @__NO_SIDE_EFFECTS__ */ () => {};
/* @__NO_SIDE_EFFECTS__ */
export function replacedFunction() {
  return 1;
}
export function replacedEmpty() {}
export const kept = /* @__NO_SIDE_EFFECTS__ */ () => {};

eval(`
  replacedLet = () => globalThis.calls.push('let');
  replacedVar = () => globalThis.calls.push('var');
  replacedFunction = () => globalThis.calls.push('function');
  replacedEmpty = () => globalThis.calls.push('empty function');
`);
