import assert from "node:assert";

import("./c").then((mod) => {
  assert.strictEqual(mod.default()(), 'f')
});

import("./a").then((mod) => {
  assert.strictEqual(mod.default(), 'f')
});