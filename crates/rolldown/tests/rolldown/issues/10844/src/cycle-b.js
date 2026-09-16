import { hub } from './cycle-a.js';

export function b() {
  return 'b';
}

export function lazy() {
  return import('./cycle-a.js');
}

export function eager() {
  return hub();
}
