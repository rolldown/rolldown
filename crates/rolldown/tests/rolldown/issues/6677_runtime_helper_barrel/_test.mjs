import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const distDir = path.join(import.meta.dirname, 'dist');
const assets = Object.fromEntries(
  fs.readdirSync(distDir).map((file) => [file, fs.readFileSync(path.join(distDir, file), 'utf8')]),
);

assert('components.js' in assets, 'barrels that own runtime helper state must remain emitted');
assert(
  assets['components.js'].includes('__exportAll'),
  'runtime helper usage should stay anchored in the barrel chunk',
);
