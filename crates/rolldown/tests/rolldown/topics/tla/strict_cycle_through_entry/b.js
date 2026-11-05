import { log } from './log.js';
// The cycle `main.js -> b.js -> main.js`: `main.js` reaches the TLA of `a.js`, so `b.js` reaches it too.
import './main.js';

log.push('b');
