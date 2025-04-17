# Reason
1. we don't support no bundle mode, rest part should be same
# Diff
## /out.js
### esbuild
```js
import * as ns from "./foo";
let foo = 234;
console.log(foo);
```
### rolldown
```js
import "./foo";

//#region entry.js
let foo = 234;
console.log(foo);

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,3 @@
-import * as ns from "./foo";
+import "./foo";
 let foo = 234;
 console.log(foo);

```