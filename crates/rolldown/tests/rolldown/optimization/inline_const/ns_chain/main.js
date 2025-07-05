import assert from "node:assert";
import * as ns from "./reexport.js";

assert.equal(ns.another.foo, "foo");

