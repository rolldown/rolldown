import assert from 'node:assert';
import nodeFs from 'node:fs';
import { value } from './child';
import { value2 } from './child2';

if (import.meta.hot) {
  import.meta.hot.accept();
}

if (value2 === 'child2-updated') {
  assert.deepStrictEqual(globalThis.records, [
    'child-handleable',
    'child-unhandleable',
    ['value2'],
  ]);
  nodeFs.writeFileSync('./ok-2', '');
} else if (value === 'child-handleable') {
  throw new Error(
    "This change should be handled by child itself. It shouldn't propagate to parent.",
  );
} else if (value === 'child-unhandleable') {
  assert.deepStrictEqual(globalThis.records, [
    'child-handleable',
    'child-unhandleable',
  ]);
  nodeFs.writeFileSync('./ok-1', '');
}
