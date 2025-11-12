import assert from "assert";
import * as ns from "./import.js";

assert.deepStrictEqual(Object.getOwnPropertyNames(ns), ["default"]);
assert.equal(String(ns), "[object Module]");
