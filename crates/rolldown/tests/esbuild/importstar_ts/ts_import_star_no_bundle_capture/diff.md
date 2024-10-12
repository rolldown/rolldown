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

//#region entry.ts
let foo = 234;
console.log(ns, ns.foo, foo);

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
+var foo = 234;
 console.log(ns, ns.foo, foo);

```