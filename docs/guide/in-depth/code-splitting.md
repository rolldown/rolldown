# Code Splitting

Code splitting is the process of creating chunks from modules. This chapter describes its behavior and the principles behind it.

- Code splitting is not controllable. It runs following certain rules.
- Thus, we will also refer to it as _`automatic chunking`_ against _`manual chunking`_ done by `advancedChunks`.

## `Entry chunk`s

- Code splitting will combine modules connected statically into a chunk
- `statically` means static `import ... from '...'` or `require(...)`. For example,

We have 2 entries here

- `entry.js`
- `dyn-entry.js`

```js
// entry.js
import foo from './foo.js';
import('./dyn-entry.js');

// dyn-entry.js
require('./bar.js');

// foo.js
export default 'foo';

// bar.js
module.exports = 'bar';
```

- `import('./dyn-entry.js')` creates a dynamic entry, whose entry module is `dyn-entry.js`
- `entry.js` imports `dyn-entry.js` via `import(...)` which is a dynamic import. People use it to load code on demand, so we don't want to put imported code together with importers and we don't consider them as connected statically.
- There're two groups of statically connected modules. Group 1: `entry.js` and `foo.js`. Group 2: `dyn-entry.js` and `bar.js`
- Since there are two groups, in the end, we will generate two chunks. These chunks are created due to as entries, they are also called as `entry chunk`s.
- `Entry chunk`s include `initial chunk`s and `dynamic chunk`s. They are easy to be distinguished. `initial chunk`s are generated due to users' configuration. `input: ['./a.js', './b.js']` defines two `initial chunk`s. `Dynamic chunk`s are created due to dynamic imports.

## `Common chunk`s

- Defining entries is not the only way to create chunks. When a module gets statically imported by at least two different entries, it gets pulled into a separate chunk.
- What **`automatic chunking`** tries to do here are:
- Ensure every JavaScript module is singleton in the final bundle output.
- When a entry gets executed, only imported modules should get executed.

For input

```js
// entry-a.js
import 'shared-by-ab.js';
import 'shared-by-abc.js';
console.log(globalThis.value);

// entry-b.js
import 'shared-by-ab.js';
import 'shared-by-bc.js';
import 'shared-by-abc.js';
console.log(globalThis.value);

// entry-c.js
import 'shared-by-bc.js';
import 'shared-by-abc.js';
console.log(globalThis.value);

// shared-by-ab.js
globalThis.value = globalThis.value || [];
globalThis.value.push('ab');

// shared-by-bc.js
globalThis.value = globalThis.value || [];
globalThis.value.push('bc');

// shared-by-abc.js
globalThis.value = globalThis.value || [];
globalThis.value.push('abc');
```

we get

::: code-group

```js [entry-a.js]
import './common-ab.js';
import './common-abc.js';
```

```js [entry-b.js]
import './common-ab.js';
import './common-bc.js';
import './common-abc.js';
```

```js [entry-c.js]
import './common-bc.js';
import './common-abc.js';
```

```js [common-ab.js]
globalThis.value = globalThis.value || [];
globalThis.value.push('ab');
```

```js [common-bc.js]
globalThis.value = globalThis.value || [];
globalThis.value.push('bc');
```

```js [common-abc.js]
globalThis.value = globalThis.value || [];
globalThis.value.push('abc');
```

:::

- We all know how `entry chunk`s got created. Let's talk about these 3 `common chunk`s.
- `common-ab.js` is created because `shared-by-ab.js` is imported by both `entry-a.js` and `entry-b.js`.
- `common-bc.js` is created because `shared-by-bc.js` is imported by both `entry-b.js` and `entry-c.js`.
- `common-abc.js` is created because `shared-by-abc.js` is imported by all 3 entries.
- And most importantly, whether modules could be put into the same `common chunk` is determined if they got imported by the same entries.
- This behavior ensures previous principles:
  - Every JavaScript module is singleton in the final bundle output.
  - When a entry gets executed, only imported modules should get executed.

- Why not put shared modules into the same `common chunk`?

- Executing each entry separately for above output, we get:
- `entry-a.js` emits `['ab', 'abc']`
- `entry-b.js` emits `['ab', 'bc', 'abc']`
- `entry-c.js` emits `['bc', 'abc']`

- If we put `shared-by-ab.js`, `shared-by-bc.js` and `shared-by-abc.js` into the same `common chunk` like

```js [common-all.js]
globalThis.value = globalThis.value || [];
globalThis.value.push('ab');
globalThis.value = globalThis.value || [];
globalThis.value.push('bc');
globalThis.value = globalThis.value || [];
globalThis.value.push('abc');
```

- Then we all get `['ab', 'bc', 'abc']` for executing each entry, which totally violates the intention of the original code.

## Module Placing Order

- Rolldown tries to place your modules in the order that they are declared in the original place. For example,

```js
// entry.js
import { foo } from './foo.js';
console.log(foo);

// foo.js
export var foo = 'foo';
```


- Rolldown will try to calculate the order by emulating the execution, starting from entries.
- In this case, the execution order is `[foo.js, entry.js]`;

- So the bundle output will be like

```js [output.js]
// foo.js
var foo = 'foo';

// entry.js
console.log(foo);
```

### Respecting Execution Order doesn't take precedence

- However, rolldown sometimes places modules without respecting their original order, because ensuring modules are singleton takes precedence over placing modules in the order they are declared.

For example,

```js
// entry.js
import './setup.js';
import './execution.js';

import('./dyn-entry.js');

// setup.js
globalThis.value = 'hello, world';

// execution.js
console.log(globalThis.value);

// dyn-entry.js
import './execution.js';
```

We get

::: code-group

```js [entry.js]
import './common-execution.js';

// setup.js
globalThis.value = 'hello, world';
```

```js [dyn-entry.js]
import './common-execution.js';
```

```js [common-execution.js]
console.log(globalThis.value);
```

:::

- `common-execution.js` is created because `execution.js` is imported by both `entry.js` and `dyn-entry.js`.
- This example shows the problem, before bundling, the code outputs `hello, world`, but after bundling, it outputs `undefined`.
- Currently, there's no easy way to solve this problem, as well for other bundlers that output esm.

::: info

- https://github.com/evanw/esbuild/issues/399
- https://github.com/rollup/rollup/issues/4539
  :::

- There are some discussions on how to solve this problem, one way is to create more `common chunk`s once a module violates its original order. But this will create more `common chunk`s, which is not a good idea.
- Rolldown tries to solve this issue by `InputOptions#experimental#strictExecutionOrder`, which injects some helper code to ensure the execution order is respected with keeping esm output and not creating more `common chunk`s.
