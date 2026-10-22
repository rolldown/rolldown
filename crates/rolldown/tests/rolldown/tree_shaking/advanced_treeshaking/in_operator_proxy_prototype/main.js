globalThis.log = [];
const proxy = new Proxy(
  {},
  {
    has(_target, key) {
      globalThis.log.push(key);
      return false;
    },
  },
);
const o = { __proto__: proxy };
'x' in o;
'y' in o;
export const done = true;
