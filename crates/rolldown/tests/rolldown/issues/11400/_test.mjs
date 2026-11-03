import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { card, note } from './dist/index.js';

const dist = path.join(path.dirname(fileURLToPath(import.meta.url)), 'dist');

assert.equal(card, 'card');
assert.equal(note, 'ts');

for (const file of ['index.js', 'card.js', 'card.json.js', 'note.js']) {
  assert.ok(fs.existsSync(path.join(dist, file)), `missing ${file}`);
}

assert.equal(fs.existsSync(path.join(dist, 'card2.js')), false);
assert.equal(fs.existsSync(path.join(dist, 'note.ts.js')), false);

const cardSource = fs.readFileSync(path.join(dist, 'card.js'), 'utf8');
assert.match(cardSource, /from ['"]\.\/card\.json\.js['"]/);
