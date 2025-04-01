# Reason
1. different chunk naming style
# Diff
## /out/a.js
### esbuild
```js
import {
  foo
} from "./chunk-25TWIR6T.js";

// a.js
console.log(foo);
```
### rolldown
```js
import { foo } from "./shared.js";

//#region a.js
console.log(foo);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,2 +1,2 @@
-import {foo} from "./chunk-25TWIR6T.js";
+import {foo} from "./shared.js";
 console.log(foo);

```
## /out/b.js
### esbuild
```js
import {
  foo
} from "./chunk-25TWIR6T.js";

// b.js
console.log(foo);
```
### rolldown
```js
import { foo } from "./shared.js";

//#region b.js
console.log(foo);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,2 +1,2 @@
-import {foo} from "./chunk-25TWIR6T.js";
+import {foo} from "./shared.js";
 console.log(foo);

```