import { log } from './log.js';

await import('./a.js').catch(() => log.push('import(a) rejects'));
const ns = await import.defer('./x.js');
log.push('import.defer resolves');
let result;
try {
  result = ns.value;
} catch (e) {
  result = e.message;
}
export { log, result };
