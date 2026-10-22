import assert from 'node:assert';
import png from './a.png' with { type: 'text' };
import txt from './a.txt' with { type: 'text' };

assert.strictEqual(png, 'png\n');
assert.strictEqual(txt, 'txt\n');
