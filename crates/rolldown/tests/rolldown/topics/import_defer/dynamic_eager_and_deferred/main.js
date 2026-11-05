import { log } from './log.js';

const deferred = await import.defer('./d.js');
log.push('after import.defer');
const eager = await import('./d.js');
log.push('after import');
export const values = [eager.value, deferred.value];
export const same = deferred === eager;
export { log };
