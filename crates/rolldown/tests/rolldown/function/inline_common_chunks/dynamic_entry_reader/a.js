import { s } from './shared.js';
globalThis.events.push('A ' + s);
await import('./lazy.js');
