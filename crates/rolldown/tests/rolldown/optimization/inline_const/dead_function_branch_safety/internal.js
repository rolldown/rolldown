export function dead() {
  throw new Error('dead helper retained');
}
export function live() {
  return 'live';
}
export function mutableHelper() {
  return 'mutable';
}
export const object = {
  method() {
    return this;
  },
};
