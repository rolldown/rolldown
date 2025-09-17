import assert from "node:assert";
import fs from "node:fs";
import path from "node:path";

const file = fs.readFileSync(path.resolve(import.meta.dirname, "./dist/main.js"), "utf-8");

assert(file.includes(`assert.strictEqual("immutable_let", "immutable_let")`));
assert(file.includes(`assert.strictEqual(mutable_let, "mutable_let1")`))
assert(file.includes(`assert.strictEqual(character, 0)`))
assert(file.includes(`assert.strictEqual(character, 1)`))
