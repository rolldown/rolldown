Consider the following input files:

::: code-group

```js [lib.js]
export class Test {}
console.log(Test.name); // Expected: "Test"

export function test() {}
console.log(test.name); // Expected: "test"
```

```js [main.js (entry)]
import { Test as T, test as t } from './lib';

export class Test extends T {}

export function test() {}
```

:::

Output with `keepNames: false` (default):

```js
var Test$1 = class {};
console.log(Test$1.name); // "Test$1" - not the original name!
function test$1() {}
console.log(test$1.name); // "test$1" - not the original name!

var Test = class extends Test$1 {};
function test() {}

export { Test, test };
```

Output with `keepNames: true`:

```js
// NOTE: `__name` is a helper function that sets `name` property

var Test$1 = class {
  static {
    __name(this, 'Test');
  }
};
console.log(Test$1.name); // "Test" - preserved!
function test$1() {}
__name(test$1, 'test');
console.log(test$1.name); // "test" - preserved!

var Test = class extends Test$1 {};
function test() {}

export { Test, test };
```
