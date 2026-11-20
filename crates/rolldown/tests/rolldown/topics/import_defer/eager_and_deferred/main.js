import { log } from './log.js';
import defer * as ns from './x.js';
import './y.js';

// The eager import in `y.js` evaluates `x.js` at its position.
export const order = [...log];
export const value = ns.foo;
