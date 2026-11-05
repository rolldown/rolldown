import assert from 'node:assert';

const load = async () => {
  import('./imp').then((m) => {
    assert.strictEqual(m.imp, 1);
  });
  import('./imp-other').then((m) => {
    assert.strictEqual(m.other, 'other');
  });
};

load();
