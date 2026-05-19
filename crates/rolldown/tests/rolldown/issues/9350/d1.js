import { marker } from './main.js';
import './shared.js';

console.log('d1', marker);

export function load2FromD1() {
  return import('./d2.js');
}
