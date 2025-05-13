/** @license foo1 */
const foo1 = '';

function dummy1() {}

/** foo2 */
const foo2 = '';

export default class {
  foo() {
    console.log(foo1, dummy1, foo2);
  }
  /** bar */
  bar() {
    console.log('bar' /** bar2 */);
  }
}
