import { strictEqual } from 'node:assert';
export const a = globalThis.a;
import './dynamic.js'
import './sideeffects.js'

strictEqual(a, 2)
