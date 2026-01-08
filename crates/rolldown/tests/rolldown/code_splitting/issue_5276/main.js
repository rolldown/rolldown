import assert from 'node:assert';

const load = async () => {
  import('./imp1').then((m) => {
    assert.strictEqual(m.imp1, 1);
  });
  import('./imp2').then((m) => {
    assert.strictEqual(m.imp2, 2);
  });
  import('./imp3').then((m) => {
    // When m is imported from a facade chunk, plain object assertion would fail since it has StringTag `Module`
    // use JSON serialization as a workaround
    assert.deepEqual(JSON.parse(JSON.stringify(m)), { imp3: 3, imp33: 33 });
  });
};

load();
