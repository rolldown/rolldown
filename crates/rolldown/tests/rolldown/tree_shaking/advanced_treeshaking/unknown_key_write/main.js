const o = { a: 1, b: 2 };
export function set(key) {
  o[key] = 3;
}
export function get() {
  return o.b === 2 ? 'two' : 'other';
}
