# Diff
## /out.js
### esbuild
```js
// entry.ts
import { foo } from "pkg";
var used = foo.used;
export {
  used
};
```
### rolldown
```js
import { foo } from "pkg";

//#region entry.ts
var used = foo.used;
var unused = foo.unused;

//#endregion
export { used };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,4 @@
 import {foo} from "pkg";
 var used = foo.used;
+var unused = foo.unused;
 export {used};

```