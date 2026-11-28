import log from './log.cjs';
import { fooA, selfA } from './a.js';
import { getB, selfB } from './b.js';
import { getC1, getC2 } from './c.js';
import { getM } from './m.mjs';
import defer * as wrapped from './wrapped.js';

export const before = [...log];
export const results = {
  bIsC1: getB() === getC1(),
  c1IsC2: getC1() === getC2(),
  sameReceiver: selfA() === selfB(),
  foo: fooA(),
  defaultTypes: [typeof getB().default, typeof getM().default],
  wrappedFoo: wrapped.getW().foo,
};
export const after = [...log];
export { getB };
export { one, two } from './c.js';
