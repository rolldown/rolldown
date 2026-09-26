import { count, create } from './index.js';

if (count(create()) !== 2) {
  throw new Error('unexpected result');
}
