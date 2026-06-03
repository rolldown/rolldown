import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const distDir = path.join(import.meta.dirname, 'dist');
const assets = Object.fromEntries(
  fs.readdirSync(distDir).map((file) => [file, fs.readFileSync(path.join(distDir, file), 'utf8')]),
);

assert('components.js' in assets, 'wrapped CommonJS barrels must remain emitted');
assert(
  assets['components.js'].includes('__commonJS'),
  'the CommonJS wrapper should stay in the barrel chunk',
);
