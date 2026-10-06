import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert';

const bundledFile = fs.readFileSync(path.resolve(import.meta.dirname, 'dist/main.js'));
// #5557: an import of a `.`-prefixed file must keep its `./` prefix.
assert(bundledFile.includes('./.x.js'));
