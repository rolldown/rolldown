import { log } from './log.js';

// Never called. This `import.defer()` site keeps `d.js` in its own chunk, and that chunk does not
// evaluate `d.js` when it loads.
export const loadDeferred = () => import.defer('./d.js');

const ns = await import('./d.js');
log.push('after import');
export const value = ns.value;
export { log };
