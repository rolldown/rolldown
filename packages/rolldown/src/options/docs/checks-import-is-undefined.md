```js displayName="main.js"
import * as utils from './utils.js'; // 'nonExistent' is not exported
console.log(utils.nonExistent); // Always undefined
```

```js displayName="utils.js"
export const helper = () => 'help';
```
