import assert from "node:assert";

const load = async () => {
  import("./imp1").then((m) => {
    assert.strictEqual(m.imp1, 1);
  });
  import("./imp2").then((m) => {
    assert.strictEqual(m.imp2, 2);
  });
  import("./imp3").then((m) => {
    assert.deepEqual(m, { imp3: 3, imp33: 33 });
  });
};

load();
