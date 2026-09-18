import { s } from './shared.js';
const m = await import('./lazy.js');
globalThis.events.push('A ' + s + m.s);
