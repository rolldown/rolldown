// Inline export: split per declarator before scanning, `A`/`B` shaken.
export let F = () => {},
  A = () => B(),
  B = () => A();

// Export list: the statement must be split the same way so `a`/`b` are shaken.
let f = () => {},
  a = () => b(),
  b = () => a();

// Mixed destructuring/simple bindings must not be split. The array binding can
// perform iterator work even if only `used` is demanded by the export list.
let iterated = false;
const iterable = {
  [Symbol.iterator]() {
    iterated = true;
    return [][Symbol.iterator]();
  },
};
const [unused] = iterable,
  used = iterated;

export { f, a, b, used };
