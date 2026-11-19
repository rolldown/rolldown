export function eager() {
  return 1;
}
export function load() {
  return import('./a.mjs');
}
