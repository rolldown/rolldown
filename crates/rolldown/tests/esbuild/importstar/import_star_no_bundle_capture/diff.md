# Diff
## /out.js
### esbuild
```js
import * as ns from "./foo";
let foo = 234;
console.log(ns, ns.foo, foo);
```
### rolldown
```js
import * as ns from "./foo";

//#region entry.js
let foo$1 = 234;
console.log(ns, ns.foo, foo$1);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
 import * as ns from "./foo";
-let foo = 234;
-console.log(ns, ns.foo, foo);
+let foo$1 = 234;
+console.log(ns, ns.foo, foo$1);

```