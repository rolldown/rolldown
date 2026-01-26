::: code-group

```js [main.js]
import * as utils from './utils.js';

// This will trigger the warning
// because `utils` is a namespace object, not a function
utils();
```

```js [utils.js]
export function greet() {
  return 'Hello';
}
```

:::
