import { log } from './log.js';

const ns = await import.defer('./d.js');
log.push('after import');
export const keys = Object.keys(ns);
log.push('after keys');
export const value = ns.value;
export const then = 'then' in ns;
export { log };
