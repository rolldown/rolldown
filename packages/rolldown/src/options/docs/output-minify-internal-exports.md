#### In-depth

For example, if you have the following code:

```js
// main.js
import './lib.js';

// lib.js
import('./dynamic.js');
export const importantValue = 42;

// dynamic.js
import { importantValue } from './lib.js';
console.log(importantValue);
```

The output with `minifyInternalExports: false` will be:

```js
// main.js
import('./dynamic-CCJ-yTfk.js');
const importantValue = 42;

export { importantValue };

// dynamic-CCJ-yTfk.js
import { importantValue } from './index.js';

console.log(importantValue);
```

On the other hand, the output with `minifyInternalExports: true` will be:

```js
// main.js
import('./dynamic-CCJ-yTfk.js');
const importantValue = 42;

export { importantValue as t };

// dynamic-CCJ-yTfk.js
import { t as importantValue } from './index.js';

console.log(importantValue);
```

Even though it appears that setting this option to `true` makes the output larger, it actually makes it smaller if a minifier is used. In this case, `export { importantValue as t }` can become e.g., `export{t as e}` or even `export{t}`, while otherwise it would produce `export{ a as importantValue }` because a minifier usually will not change export signatures.
