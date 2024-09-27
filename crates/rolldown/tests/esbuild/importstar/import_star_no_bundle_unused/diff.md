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

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,3 +1,3 @@
-import * as ns from './foo';
-let foo = 234;
+import './foo';
+var foo = 234;
 console.log(foo);
\ No newline at end of file

```
