import assert from "node:assert";
import fn from "./dep.js";
import cls from "./dep-class.js";

var count = 100;
// With keepNames enabled, the name of an anonymous default export function should be "default"
assert.strictEqual(
  fn.name,
  "default",
  'Anonymous default export function name should be "default"',
);

// With keepNames enabled, the name of an anonymous default export class should be "default"
assert.strictEqual(
  cls.name,
  "default",
  'Anonymous default export class name should be "default"',
);
export default class {}

assert.strictEqual(count, 100);
