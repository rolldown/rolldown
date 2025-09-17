import assert from 'node:assert';
import { value } from './child';
import { value as parent2ChildValue } from './parent2-child';

assert(['child', 'child-updated'].includes(value));
assert.strictEqual(parent2ChildValue, 'parent2-child');
nodeFs.writeFileSync('./ok-1', '');

if (import.meta.hot) {
  import.meta.hot.accept();
}
