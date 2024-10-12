# Diff
## /out.js
### esbuild
```js
export { default as foo } from "./foo";
export { default as bar } from "./bar";
```
### rolldown
```js
import { default as foo } from "./foo";
import { default as bar } from "./bar";

export { bar, foo };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,2 +1,3 @@
-export {default as foo} from "./foo";
-export {default as bar} from "./bar";
+import {default as foo} from "./foo";
+import {default as bar} from "./bar";
+export {bar, foo};

```