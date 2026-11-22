import { inner } from './inner.js';
globalThis.events.push('E ' + inner());
