import log from './log.cjs';
import defer * as ns from './t.cjs';

// `set` returns false and evaluates nothing (spec: [[Set]] of a module namespace).
let setError;
try {
  ns.foo = 'bar';
} catch (e) {
  setError = e;
}
export const setThrows = setError instanceof TypeError;
export const afterSet = [...log];

// `delete` evaluates the module, and then returns false for an export.
let deleteError;
try {
  delete ns.foo;
} catch (e) {
  deleteError = e;
}
export const deleteThrows = deleteError instanceof TypeError;
export const afterDelete = [...log];

// A write to a property of an export reads the export first.
ns.obj.value = 2;
export const objValue = ns.obj.value;

// Each call form keeps the namespace object as `this`.
const isNamespace = (value) => value !== undefined && value !== globalThis && 'self' in value;
export const calls = [
  isNamespace(ns.self()),
  isNamespace(ns.self?.()),
  isNamespace(ns?.self()),
  isNamespace(ns['self']()),
  isNamespace(ns.tag`x`),
];
