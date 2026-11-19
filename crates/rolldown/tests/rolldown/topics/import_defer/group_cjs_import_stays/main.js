globalThis.__log = [];
import defer * as r from './r.js';
import defer * as s from './s.js';

export function run() {
  return r.read();
}

export function runS() {
  return s.s;
}
