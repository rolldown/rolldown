import './b.js';
import { enabled } from './c.js';
if (enabled) await 0;
(globalThis.tlaOrder ??= []).push('a');
