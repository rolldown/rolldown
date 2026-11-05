import a from './a.cjs';
import './after.js';

console.log(globalThis.__events.join(), a.seen);
