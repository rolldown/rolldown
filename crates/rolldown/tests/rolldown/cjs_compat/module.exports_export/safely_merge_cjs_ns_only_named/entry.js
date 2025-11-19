// Test case for SafelyMergeCjsNs optimization with only named and default imports
import assert from 'node:assert';
import { useState } from 'this-is-only-used-for-testing';

assert.deepStrictEqual(typeof useState, 'function');
