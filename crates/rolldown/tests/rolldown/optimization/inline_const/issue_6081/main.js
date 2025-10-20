import { version } from "./react.js";
import { default as render } from "./should_not_inline_import_default.js";
import assert from "node:assert";

const majorIsOne = version.startsWith("1");

assert.strictEqual(majorIsOne, true);
assert.strictEqual(render, 1000);
