const iterable = (name) => ({
  [Symbol.iterator]() {
    globalThis.calls.push(name);
    return [][Symbol.iterator]();
  },
});

/* @__NO_SIDE_EFFECTS__ */
export function make(name) {
  return iterable(name);
}
export const plain = (name) => iterable(name);
export const imported = iterable('imported');
