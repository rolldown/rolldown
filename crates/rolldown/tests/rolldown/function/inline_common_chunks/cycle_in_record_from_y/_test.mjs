import assert from 'node:assert';

globalThis.events = [];
await import('./dist/b.js');
await import('./dist/a.js');

assert.deepStrictEqual(globalThis.events, ['X body yv', 'Y body x(y) xv', 'B yyv', 'A x(y)']);
