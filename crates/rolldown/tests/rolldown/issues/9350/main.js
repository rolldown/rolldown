export const marker = 'main';

export function load1() {
  return import('./d1.js');
}

export function load2() {
  return import('./d2.js');
}

console.log('main');
