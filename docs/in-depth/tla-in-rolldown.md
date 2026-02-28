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

One downside of TLA in rolldown is that it will change the original code's behavior from concurrent to sequential. It still ensures the relative order, but indeed slows down the execution and may break the execution if the original code relies on concurrency.

```dot
digraph {
    bgcolor="transparent";
    rankdir=LR;
    node [shape=box, style="filled,rounded", fontname="Arial", fontsize=12, margin="0.2,0.1", color="${#3c3c43|#dfdfd6}", fontcolor="${#3c3c43|#dfdfd6}"];
    edge [fontname="Arial", fontsize=10, color="${#3c3c43|#dfdfd6}", fontcolor="${#3c3c43|#dfdfd6}"];
    compound=true;

    subgraph cluster_before {
        label="Before bundling (concurrent)";
        labeljust="l";
        fontname="Arial";
        fontsize=12;
        fontcolor="${#3c3c43|#dfdfd6}";
        style="dashed,rounded";
        color="${#22863a|#3fb950}";

        b_main [label="main.js\nimport tla1, tla2", fillcolor="${#fff0e0|#4a2a0a}"];
        b_all [label="Promise.all([\n  tla1,\n  tla2\n])", fillcolor="${#dcfce7|#14532d}"];
        b_tla1 [label="tla1.js\nawait ...", fillcolor="${#dbeafe|#1e3a5f}"];
        b_tla2 [label="tla2.js\nawait ...", fillcolor="${#dbeafe|#1e3a5f}"];
        b_done [label="both resolved", fillcolor="${#dcfce7|#14532d}"];

        b_main -> b_all;
        b_all -> b_tla1;
        b_all -> b_tla2;
        b_tla1 -> b_done;
        b_tla2 -> b_done;
    }

    subgraph cluster_after {
        label="After bundling (sequential)";
        labeljust="l";
        fontname="Arial";
        fontsize=12;
        fontcolor="${#3c3c43|#dfdfd6}";
        style="dashed,rounded";
        color="${#d44803|#ff712a}";

        a_tla1 [label="await tla1", fillcolor="${#dbeafe|#1e3a5f}"];
        a_tla2 [label="await tla2", fillcolor="${#dbeafe|#1e3a5f}"];
        a_main [label="console.log(\n  foo1, foo2\n)", fillcolor="${#fff0e0|#4a2a0a}"];

        a_tla1 -> a_tla2 [label="then"];
        a_tla2 -> a_main [label="then"];
    }
}
```

A real-world example would looks like

```js
// main.js
import { bar } from './sync.js';
import { foo1 } from './tla1.js';
import { foo2 } from './tla2.js';
console.log(foo1, foo2, bar);

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
