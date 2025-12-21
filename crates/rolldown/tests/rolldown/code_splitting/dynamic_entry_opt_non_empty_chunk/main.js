import assert from "node:assert";

// This test case verifies that dynamic entry chunks with their own modules
// are NOT merged (only empty facade chunks should be merged)
const load = async () => {
  import("./entry-with-code").then((m) => {
    assert.strictEqual(m.value, 42);
  });
};

load();
