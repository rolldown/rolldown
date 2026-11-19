/*! legal before a removed import */
import { helper } from './helper.js';

/*! legal before a hoisted function */
export function describe() {
  return helper('m');
}

export const made = /* @__PURE__ */ make();

function make() {
  return { made: true };
}

/*! legal in the body */
globalThis.__log = ['m'];
