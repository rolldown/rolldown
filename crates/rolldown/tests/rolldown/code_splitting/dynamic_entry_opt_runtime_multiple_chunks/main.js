import assert from "node:assert";

// Test where runtime needs to be assigned to a separate chunk
// because multiple common chunks are optimized
const load = async () => {
  import("./module-a").then((m) => {
    assert.strictEqual(m.valueA, 'a');
  });
  
  import("./module-b").then((m) => {
    assert.strictEqual(m.valueB, 'b');
  });
  
  import("./module-c").then((m) => {
    assert.strictEqual(m.valueC, 'c');
  });
};

load();
