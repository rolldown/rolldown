import { identity } from './lib.js';

identity(Object);
globalThis.calls.push('ready');
