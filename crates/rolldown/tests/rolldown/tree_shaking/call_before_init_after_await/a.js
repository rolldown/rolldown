import { f } from './main.js';

await 0;
f();
(globalThis.calls ??= []).push('a done');
