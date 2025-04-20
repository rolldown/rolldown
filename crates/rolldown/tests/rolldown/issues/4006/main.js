import a from './a.js';
import b from './b.js';
/**
 * named export function
 */
export function foo() {
  return a + b
}
/**
 * named export class
 */
export class Bar {}
/**
 * named export const decl
 */
export var Baz = 'baz';
/**
 * default export expr
 */
export default Baz;
/**
 * stmt
 */
console.log(666);

