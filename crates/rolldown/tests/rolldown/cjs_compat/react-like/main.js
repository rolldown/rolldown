import React from "./commonjs.js";
import { version } from "./commonjs2.js";

import assert from "node:assert";

assert.equal(React.createReactElement(), "div");
assert.equal(React.version.toString(), '1');
assert.equal(version.toString(), '1');
