export function used() {
  return 1;
}
export function deadA() {
  return import(`./a.mjs`);
}
export function deadB() {
  return import(`./b.mjs`);
}
