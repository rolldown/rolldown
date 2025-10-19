import assert from 'node:assert';
import nodeFs from 'node:fs';
export { value as fooValue } from './foo';

if (import.meta.hot) {
  import.meta.hot.accept((newExports) => {
    assert.equal(newExports.fooValue, 'edited-foo-called');
    nodeFs.writeFileSync('./ok-0', '');
  });
}
