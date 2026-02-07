import { strictEqual } from 'node:assert';
import { add, multiply } from './utils.js';
import { compute } from './app.js';

strictEqual(add(2, 3), 5);
strictEqual(multiply(4, 5), 20);
strictEqual(compute(2, 3), 11); // add(2,3) + multiply(2,3) = 5 + 6 = 11
