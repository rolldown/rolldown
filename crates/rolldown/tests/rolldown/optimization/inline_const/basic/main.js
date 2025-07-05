import assert from "node:assert";
import { foo } from "./foo.js";
import * as ns from "./ns.js";
import cjs from "./cjs.js";

assert.equal(foo, "foo");
assert.equal(ns.a, "a");
assert.equal(cjs.foo, "cjs-foo");

