// Test case with SEPARATE import statements for named and default imports
import assert from 'node:assert';
import { useState } from 'this-is-only-used-for-testing';
import React from 'this-is-only-used-for-testing';

assert.deepStrictEqual(typeof React, 'function');
assert.deepStrictEqual(React, React.default);
assert.deepStrictEqual(typeof useState, 'function');
