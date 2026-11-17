import { e as outer, g as blockOuter } from './renamed.js';
import * as self from './renamed.js';

export const log = [];
export var e = 'v';
e += '!';
try {
  throw 1;
} catch (e) {
  var e = 2;
  log.push(outer);
}
{
  let e = 3;
  e++;
  log.push(outer, e);
}

export var f = 'w';
f += '!';
try {
  throw 1;
} catch (f) {
  var f = 4;
  log.push(self.f);
}
{
  let f = 5;
  f++;
  log.push(self.f, f);
}

export var g = 8;
g++;
try {
  throw 0;
} catch (g) {
  var g = 10;
  log.push(g);
}
{
  let g = 11;
  g++;
  log.push(blockOuter, g);
}
