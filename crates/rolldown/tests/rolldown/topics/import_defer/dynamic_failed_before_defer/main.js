import { log } from './log.js';

await import('./d.js').catch((e) => log.push(`import(d) rejects: ${e.message}`));
// `d.js` failed. The walk of `import.defer()` stops at a failed module, so `a.js` does not start.
const ns = await import.defer('./d.js');
log.push('import.defer resolves');
let result;
try {
  result = ns.value;
} catch (e) {
  result = e.message;
}
export { log, result };
