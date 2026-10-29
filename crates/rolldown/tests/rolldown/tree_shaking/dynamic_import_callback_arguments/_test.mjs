import assert from 'node:assert/strict';
const { namedArguments, inheritedArguments, ...loaders } = await import('./dist/main.js');
for (const [name, load] of Object.entries(loaders)) {
  assert.equal(await load(), 'expected', name);
}
assert.equal(await namedArguments(), 'expected:other');
assert.equal(await inheritedArguments('expected'), 'expected');
