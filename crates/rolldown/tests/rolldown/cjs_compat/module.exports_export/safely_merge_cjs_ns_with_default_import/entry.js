// Test case for SafelyMergeCjsNs optimization with both named and default imports
import assert from 'node:assert';
import React, { useState } from 'this-is-only-used-for-testing';

assert.deepStrictEqual(typeof React, 'function');
assert.deepStrictEqual(React, React.default);
assert.deepStrictEqual(typeof useState, 'function');
