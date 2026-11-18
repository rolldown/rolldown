import assert from 'node:assert';
import data from './data.txt' with { type: 'json' };
import text from './data.txt' with { type: 'text' };
import shader from './shader.glsl' with { type: 'text' };
import bytes from './data.bin' with { type: 'bytes' };
import config from './config.json' with { type: 'json' };
import plainConfig from './config.json';

assert.deepStrictEqual(data, { value: 1 });
assert.strictEqual(text, '{ "value": 1 }\n');
assert.strictEqual(shader, 'void main() {}\n');
assert.ok(bytes instanceof Uint8Array);
assert.deepStrictEqual([...bytes], [65, 66]);
assert.deepStrictEqual(config, { value: 2 });
assert.deepStrictEqual(plainConfig, { value: 2 });
