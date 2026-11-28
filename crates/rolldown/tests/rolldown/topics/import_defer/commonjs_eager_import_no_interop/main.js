import { log } from './log.cjs';
import './c.js';
import { yFoo } from './d.js';
import defer * as x from './x.cjs';
import defer * as y from './y.cjs';

export const before = [...log];
// A static access to each deferred namespace, and an escape of one namespace object.
export const values = [x.foo, y.foo, yFoo, typeof x];
export const after = [...log];
