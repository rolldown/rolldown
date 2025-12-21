import assert from "node:assert";

// Multiple imports to the same module with different properties
const load = async () => {
  // Import only 'a' property
  import("./shared").then((m) => {
    assert.strictEqual(m.a, 1);
  });
  
  // Import only 'b' property
  import("./shared").then((m) => {
    assert.strictEqual(m.b, 2);
  });
  
  // Import both 'a' and 'b'
  import("./shared").then((m) => {
    assert.deepEqual({ a: m.a, b: m.b }, { a: 1, b: 2 });
  });
  
  // Import the whole namespace
  import("./shared").then((m) => {
    assert.deepEqual(m, { a: 1, b: 2, c: 3 });
  });
};

load();
