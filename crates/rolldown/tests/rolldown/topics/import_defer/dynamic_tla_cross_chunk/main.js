import { log } from './log.js';

const promise = import.defer('./d.js');
log.push('after call');
const ns = await promise;
log.push('after import');
export const value = ns.value;
export { log };
