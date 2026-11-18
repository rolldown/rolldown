import assert from 'node:assert';
import { colorNs } from './css.js';

assert.strictEqual(colorNs.setOpacity('blue', 0.2), 'blue:0.2');
