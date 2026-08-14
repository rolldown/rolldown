import * as BAZ from './baz.js';
export function foo() {
  const obj = {
    bar: BAZ,
  };
  return obj;
}
export * from 'bar';
