import { execFileSync } from 'node:child_process';

for (const order of [
  ['lib', 'consumer', 'other'],
  ['consumer', 'lib', 'other'],
  ['lib', 'other', 'consumer'],
  ['consumer', 'other', 'lib'],
]) {
  const imports = order
    .map(
      (name) =>
        `import * as ${name} from ${JSON.stringify(new URL(`./dist/${name}.js`, import.meta.url).href)};`,
    )
    .join('\n');
  // Each order needs a fresh module cache to expose initialization before the base class exists.
  execFileSync(
    process.execPath,
    [
      '--input-type=module',
      '-e',
      `${imports}
      import assert from 'node:assert/strict';
      for (const instance of [lib.App.create(), consumer.default.create()]) {
        assert.ok(instance instanceof lib.Child);
        assert.ok(instance instanceof lib.Base);
        assert.equal(instance.app, lib.App);
      }
      assert.equal(other.other(), lib.Base);`,
    ],
    { encoding: 'utf8' },
  );
}
