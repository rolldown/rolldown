import { s } from './shared.js';

(globalThis.order ??= []).push('b:' + s);
