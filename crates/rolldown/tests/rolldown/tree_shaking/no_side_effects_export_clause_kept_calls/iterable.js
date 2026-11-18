/* #__NO_SIDE_EFFECTS__ */
function iterable(name) {
  return {
    *[Symbol.iterator]() {
      globalThis.calls.push(name);
    },
  };
}

export { iterable };
