import assert from 'node:assert';

const load = async () => {
  import('./imp').then((m) => {
    assert.strictEqual(m.imp, 1);
  });
};

load();
