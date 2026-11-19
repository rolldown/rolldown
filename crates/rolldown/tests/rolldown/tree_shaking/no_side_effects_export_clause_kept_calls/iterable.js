/* #__NO_SIDE_EFFECTS__ */
function iterable(name) {
  return {
    *[Symbol.iterator]() {
      globalThis.calls.push(name);
    },
  };
}

/* @__NO_SIDE_EFFECTS__ */
function coercible() {
  return {
    [Symbol.toPrimitive]() {
      globalThis.calls.push('coercion');
      return 'value';
    },
  };
}

function noop() {}

export { coercible, iterable, noop };
