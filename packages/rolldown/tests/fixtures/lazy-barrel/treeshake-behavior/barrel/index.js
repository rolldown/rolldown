export { a } from './a.js';
export { b } from './b.js';
export { c } from './c.js';
import { d, dd } from './d.js';
export { d, dd };

import './e.js';
import './f.js';
import { gg } from './g.js';

console.log('./index.js', gg);

export const index = 'index';
export default gg;