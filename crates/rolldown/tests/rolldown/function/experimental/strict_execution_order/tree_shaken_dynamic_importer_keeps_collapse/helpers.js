export function used() {
  (globalThis.log ??= []).push('used');
}

export function unused() {
  return import('./target.js');
}
