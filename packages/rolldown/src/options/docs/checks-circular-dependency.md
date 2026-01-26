::: code-group

```js [a.js]
import { b } from './b.js';
export const a = 'a' + b;
```

```js [b.js]
import { a } from './a.js';
export const b = 'b' + a;
```

```js [main.js]
import { a } from './a.js';
console.log(a);
```

:::

In this example, `a.js` imports from `b.js`, and `b.js` imports from `a.js`, creating a circular dependency.
