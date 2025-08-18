import assert from "node:assert";
import React from "this-is-only-used-for-testing";
import {test} from './lib.js'


assert.equal(React.createReactElement(), "div");
assert.equal(React.version.toString(), '1');
assert.equal(test().toString(), '1');
