import assert from "node:assert";

// Test without advancedChunks - chunks should NOT be optimized
// because target chunks are not AdvancedChunks
const load = async () => {
  import("./module1").then((m) => {
    assert.strictEqual(m.value1, 1);
  });
  
  import("./module2").then((m) => {
    assert.strictEqual(m.value2, 2);
  });
};

load();
