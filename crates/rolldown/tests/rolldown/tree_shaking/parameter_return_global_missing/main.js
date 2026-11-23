import { identity } from './lib.js';

identity(window);
globalThis.calls.push('ready');
