# Top Level Await(TLA) in Rolldown

Background knowledge:

- https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/await#top_level_await
- https://github.com/tc39/proposal-top-level-await

## How rolldown handles TLA

At this point, the principle of supporting TLA in rolldown is: we will make it work after bundling without preserving 100% semantic as the original code.

Current rules are:

- If your input contains TLA, it could only be bundled and emitted with `esm` format.
- `require` TLA module is forbidden.

## Concurrent to sequential

One downside of TLA in rolldown is that it will change the original code's behavior from concurrent to sequential. It still ensures the relative order but indeed slows down the execution.

A real-world example would looks like

```js
// main.js
import { bar } from './sync.js';
import { foo1 } from './tla1.js';
import { foo2 } from './tla2.js';
console.log(foo, foo2, bar);

// tla1.js

export const foo1 = await Promise.resolve('foo1');

// tla2.js

export const foo2 = await Promise.resolve('foo2');

// sync.js

export const bar = 'bar';
```

After bundling, it will be

```js
// tla1.js
const foo1 = await Promise.resolve('foo1');

// tla2.js
const foo2 = await Promise.resolve('foo2');

// sync.js
const bar = 'bar';

// main.js
console.log(foo1, foo2, bar);
```

You can see that, in bundled code, promise `foo1` and `foo2` are resolved sequentially, but in the original code, they are resolved concurrently.

There's a very [good example](https://github.com/tc39/proposal-top-level-await?tab=readme-ov-file#semantics-as-desugaring) of TLA spec repo, which explains the mental model of how the TLA works

```js
import { a } from './a.mjs';
import { b } from './b.mjs';
import { c } from './c.mjs';

console.log(a, b, c);
```

could be considered as the following code after desugaring:

```js
import { a, promise as aPromise } from './a.mjs';
import { b, promise as bPromise } from './b.mjs';
import { c, promise as cPromise } from './c.mjs';

export const promise = Promise.all([aPromise, bPromise, cPromise]).then(() => {
  console.log(a, b, c);
});
```

However, in rolldown, it will looks like this after bundling:

```js
import { a, promise as aPromise } from './a.mjs';
import { b, promise as bPromise } from './b.mjs';
import { c, promise as cPromise } from './c.mjs';

await aPromise;
await bPromise;
await cPromise;

console.log(a, b, c);
```
