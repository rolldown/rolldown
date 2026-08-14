import assert from 'node:assert';

const load = async () => {
  import('./imp').then((m) => {
    assert.strictEqual(m.imp, 1);
  });
  import('./imp2').then((m) => {
    assert.strictEqual(m.imp2, 2);
  });
};

load();
