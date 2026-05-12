// @ts-nocheck
import assert from 'node:assert';
import { a, b, Theme } from './dist/main';

assert.strictEqual(a, 1);
assert.strictEqual(b, 2);

// Theme.Light must still equal "Light" — the bug overwrote its value with "Default".
assert.strictEqual(Theme.Light, 'Light');
assert.strictEqual(Theme.Dark, 'Dark');
assert.strictEqual(Theme.Default, 'Light');
