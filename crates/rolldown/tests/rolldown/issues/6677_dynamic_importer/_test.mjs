import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const distDir = path.join(import.meta.dirname, 'dist');
const assets = Object.fromEntries(
  fs.readdirSync(distDir).map((file) => [file, fs.readFileSync(path.join(distDir, file), 'utf8')]),
);

assert(
  'components.js' in assets,
  'barrels that are also dynamic import targets must remain emitted',
);
assert(
  assets['index.js'].includes('import("./components.js")'),
  'dynamic import boundary should still target the barrel chunk',
);
