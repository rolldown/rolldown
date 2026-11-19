import defer * as d from './d.js';

export function run() {
  return typeof d.f;
}

export function dynamic() {
  return import('./d.js');
}
