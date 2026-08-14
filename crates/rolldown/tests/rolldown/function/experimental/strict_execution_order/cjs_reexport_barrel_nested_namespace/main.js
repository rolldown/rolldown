import * as ns from './wrapper.js';

export const result = {
  cloned: ns[globalThis.__namespaceKey]({ value: 1 }),
  cn: ns.cn('x'),
  keys: Object.keys(ns).sort(),
};
