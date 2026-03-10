import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const code = fs.readFileSync(path.join(__dirname, 'dist/main.js'), 'utf8');
const matches = code.match(/__name\(this, "Foo"\);/g) ?? [];

assert.strictEqual(matches.length, 1);
