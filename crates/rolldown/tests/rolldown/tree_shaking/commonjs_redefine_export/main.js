import { scope } from "./scope.js";
import assert from "node:assert";

assert.ok(scope instanceof WeakMap);
