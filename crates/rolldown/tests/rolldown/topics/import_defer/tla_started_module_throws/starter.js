import { log } from './log.js';

// This import starts `m1.js` before the preparation of `i.js`, and nothing awaits it.
import('./m1.js').catch(() => log.push('import(m1) rejects'));
