import assert from 'node:assert';
import nodeFs from 'node:fs';
import { value } from './child';

if (import.meta.hot) {
  import.meta.hot.accept();
}

if (value === 'child-handleable') {
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
