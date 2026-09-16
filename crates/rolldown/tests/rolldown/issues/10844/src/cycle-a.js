import { b } from './cycle-b.js';
import { m } from './messages.js';

export function hub() {
  return m.m1() + m.m2() + b();
}
