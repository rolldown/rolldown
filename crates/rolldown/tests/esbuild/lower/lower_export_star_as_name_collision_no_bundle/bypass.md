# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out.js
### esbuild
```js
import * as ns from "path";
export { ns };
let ns2 = 123;
export { ns2 as sn };
```
### rolldown
```js
import * as ns from "path";

//#region entry.js
let ns$1 = 123;
//#endregion

export { ns, ns$1 as sn };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,3 @@
 import * as ns from "path";
-export {ns};
-let ns2 = 123;
-export {ns2 as sn};
+var ns$1 = 123;
+export {ns, ns$1 as sn};

```