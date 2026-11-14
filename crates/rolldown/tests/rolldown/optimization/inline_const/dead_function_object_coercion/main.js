import { EMPTY } from './env.js';

// The runtime calls the `toString` of the prototype, so `+('1' + '')` is 1. oxc converts an
// object literal with the default `toString` and gets `+'[object Object]'`, which is NaN.
export function ifStatement() {
  if (+({ __proto__: { toString: () => '1' } } + EMPTY)) return 'consequent';
  return 'fallthrough';
}

export function conditional() {
  return +({ __proto__: { toString: () => '1' } } + EMPTY) ? 'consequent' : 'alternate';
}

export function logical() {
  return +({ __proto__: { toString: () => '1' } } + EMPTY) && 'right';
}

if (+({ __proto__: { toString: () => '1' } } + EMPTY)) globalThis.topLevel = 'consequent';
else globalThis.topLevel = 'alternate';
