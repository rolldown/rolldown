import string from './string.js';
export function first() {
  return import(/* @vite-ignore */ string);
}
export function second() {
  return import(/* @vite-ignore */ string);
}
