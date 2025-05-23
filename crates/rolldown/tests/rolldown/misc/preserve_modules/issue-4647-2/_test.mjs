import assert from "node:assert";
import { mod, a } from "./main.js";
import entry2 from "./entry2.js";

assert.strictEqual(a, "a");

assert.strictEqual((await mod()).a, 1000);

assert.strictEqual((await entry2).a, 1000);
