import assert from 'node:assert';
import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const fixtureDir = dirname(fileURLToPath(import.meta.url));
const dist = join(fixtureDir, 'dist');

assert.ok(existsSync(join(dist, 'bin', 'index.js')), 'expected dist/bin/index.js to exist');
assert.ok(existsSync(join(dist, 'lib', 'helper.js')), 'expected dist/lib/helper.js to exist');
assert.ok(
  !existsSync(join(dist, 'src', 'bin', 'index.js')),
  'did not expect dist/src/bin/index.js',
);
assert.ok(
  !existsSync(join(dist, 'src', 'lib', 'helper.js')),
  'did not expect dist/src/lib/helper.js',
);
