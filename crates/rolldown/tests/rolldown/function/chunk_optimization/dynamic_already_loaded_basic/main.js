import { value } from './shared.js';

console.log('main', value);
export function load() {
  return import('./dynamic.js');
}
