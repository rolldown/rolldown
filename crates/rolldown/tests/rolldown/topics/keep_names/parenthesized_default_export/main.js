import assert from "node:assert";

import dep_default from "./dep.js";
import dep_class_default from "./dep-class.js";
import dep_arrow_default from "./dep-arrow.js";
import dep_default_with_name from "./dep-named-default.js";

assert.strictEqual(dep_default.name, "default");
assert.strictEqual(dep_class_default.name, "default");
assert.strictEqual(dep_arrow_default.name, "default");
assert.strictEqual(dep_default_with_name.name, "name");
