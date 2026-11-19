import { replaced } from './eval.js';
import { redeclared } from './redeclared.js';
import { restDefault } from './rest-default.js';
import { coercible, iterable, noop } from './iterable.js';
import { value } from './getter.cjs';
import * as ns from './getter.cjs';

replaced();
redeclared();
restDefault();
const [first] = iterable('destructuring');
String(coercible());
noop(value);
noop(ns.value);

try {
  early();
} catch (error) {
  globalThis.calls.push(error.name);
}
const late = globalThis.calls.length;
function early() {
  noop(late);
}

iterable('removed: nothing runs on the result');
