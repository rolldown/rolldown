## /out.js
### esbuild
```js
// entry.js
import * as out from "foo";
export {
  out
};
```
### rolldown
```js
import * as out from "foo";

export { out };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs

```
