import { s, load } from './shared.js';
const m = await load();
globalThis.events.push('A ' + s + m.l);
