import assert from 'node:assert';
import nodeFs from 'node:fs';
export { value as barValue } from './bar';
export { value as fooValue } from './foo';

if (import.meta.hot) {
  import.meta.hot.accept((newExports) => {
    assert.equal(newExports.fooValue, 'edited-foo');
    assert.equal(newExports.barValue, 'edited-bar');
    nodeFs.writeFileSync('./ok-0', '');
  });
}
