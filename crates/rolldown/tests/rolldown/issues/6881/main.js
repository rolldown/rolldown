import assert from "node:assert";
const testJson = await import("./test.json").then((r) => {
  assert.deepEqual(r.default, { hello: "Hola" });
});

export { testJson };
