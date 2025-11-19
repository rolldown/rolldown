const assert = require("assert");
exports.foo = function a() {
  "use strict";
  return "foo";
};
assert.strict(exports.foo.toString().includes('"use strict";'));
