import assert from "node:assert";

// Test interaction with minChunkSize optimization
// This ensures the optimization works correctly with other chunk optimization strategies
const load = async () => {
  import("./dep1").then((m) => {
    assert.strictEqual(m.value, 'dep1');
  });
  
  import("./dep2").then((m) => {
    assert.strictEqual(m.value, 'dep2');
  });
};

load();
