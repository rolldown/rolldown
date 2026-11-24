import { x } from './counter_a.cjs';
import * as other from './other.js';
import './lazy.js';

// Code reads a property of `counter_a.cjs`.
globalThis.appX = x;
// Code reads the namespace object of `other.js`, which has a getter of `x`.
const read = (namespace) => namespace.x;
globalThis.otherX = read(other);
// `import()` of `lazy.js` gives the namespace object of `lazy.js`.
export const loadLazy = () => import('./lazy.js');
