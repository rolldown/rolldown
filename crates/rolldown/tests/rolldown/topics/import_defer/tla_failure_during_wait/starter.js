import { log } from './log.js';

// This import starts `b.js` before the preparation of `i.js`.
import('./b.js').catch(() => log.push('b rejects (import)'));
