import assert from 'node:assert/strict';
import nodeFs from 'node:fs';
export { value as barValue } from './bar';
export { value as fooValue } from './foo';

if (import.meta.hot) {
  import.meta.hot.accept((newExports) => {
    assert.deepEqual(
      newExports,
      Object.defineProperty(
        {
          fooValue: 'edited-foo',
          barValue: 'edited-bar',
        },
        Symbol.toStringTag,
        { value: 'Module' },
      ),
    );
    nodeFs.writeFileSync('./ok-0', '');
  });
}
