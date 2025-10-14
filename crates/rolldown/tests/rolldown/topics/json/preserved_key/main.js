import assert from "assert";
import mod, { _let } from "./a.json";

assert.deepStrictEqual(mod, {
  eval: true,
  arguments: false,
  valid: true,
  let: true,
  _let: "result",
  globalThis: false,
});

assert.strictEqual(_let, "result");
