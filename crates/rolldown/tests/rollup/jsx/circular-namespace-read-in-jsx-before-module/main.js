import assert from 'node:assert/strict';
import { element } from './dep.js';

export function Component() {}

assert.strictEqual(element, Component);
