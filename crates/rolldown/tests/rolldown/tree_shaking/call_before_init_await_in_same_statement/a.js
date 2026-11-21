import { f } from './main.js';

if (await Promise.resolve(true)) f();
(globalThis.calls ??= []).push('a done');
