import assert from 'node:assert';

globalThis.sideEffectLog = [];

await import('./dist/admin.js');
assert.deepStrictEqual(globalThis.sideEffectLog, ['app-admin', 'admin']);

await import('./dist/theming.js');
assert.deepStrictEqual(globalThis.sideEffectLog, ['app-admin', 'admin', 'app-theming', 'theming']);

await import('./dist/personal.js');
assert.deepStrictEqual(globalThis.sideEffectLog, [
  'app-admin',
  'admin',
  'app-theming',
  'theming',
  'palette-icon',
  'app-personal:PaletteIcon',
  'personal',
]);
