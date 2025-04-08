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
import * as ns$1 from "path";

//#region entry.js
let ns$2 = 123;

//#endregion
export { ns$1 as ns, ns$2 as sn };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,3 @@
-import * as ns from "path";
-export {ns};
-let ns2 = 123;
-export {ns2 as sn};
+import * as ns$1 from "path";
+var ns$2 = 123;
+export {ns$1 as ns, ns$2 as sn};

```