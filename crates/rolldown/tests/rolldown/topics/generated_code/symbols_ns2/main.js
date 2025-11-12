import assert from "assert";
import * as ns from "./a.js";

assert.deepStrictEqual(Object.getOwnPropertyNames(ns), ["ns"]);
assert.deepStrictEqual(Object.getOwnPropertyNames(ns.ns), ["a"]);
assert.equal(String(ns), "[object Module]");
assert.equal(String(ns.ns), "[object Module]");
