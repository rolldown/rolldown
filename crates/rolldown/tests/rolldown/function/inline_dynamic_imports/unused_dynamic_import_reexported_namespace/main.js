import { Surface, Io } from './index.js';

if (Surface.count(Io.create()) !== 2) {
  throw new Error('unexpected result');
}
