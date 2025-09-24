import assert from 'node:assert';
import nodeFs from 'node:fs';
import { value as fooValue } from './foo';

const hmr1Happened = nodeFs.existsSync('./ok-0');

if (hmr1Happened) {
  assert.strictEqual(fooValue, 'edited-foo');
  nodeFs.writeFileSync('./ok-1', '');
} else {
  assert.strictEqual(fooValue, 'foo');
}
