import assert from 'node:assert';
import nodeFs from 'node:fs';
export { value } from './common-child';

if (import.meta.hot) {
  import.meta.hot.accept((newExports) => {
    globalThis.records.push('parent-executed');
    assert.equal(newExports.value, 'common-child-updated');
    assert.deepStrictEqual(globalThis.records, ['parent-executed']);
    nodeFs.writeFileSync('./ok-0', '');
  });
}
