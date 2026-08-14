import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

const distDir = path.join(import.meta.dirname, 'dist');
const main = fs.readFileSync(path.join(distDir, 'main.js'), 'utf8');

assert(
  !main.includes('import.meta.url'),
  'There should not be an import.meta.url in the main.js file',
);
