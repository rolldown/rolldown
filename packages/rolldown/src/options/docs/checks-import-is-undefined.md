::: code-group

```js [main.js]
import * as utils from './utils.js'; // 'nonExistent' is not exported
console.log(utils.nonExistent); // Always undefined
```

```js [utils.js]
export const helper = () => 'help';
```

:::
