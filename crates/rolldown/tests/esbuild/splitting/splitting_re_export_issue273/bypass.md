# Reason
1. different chunk naming style
# Diff
## /out/a.js
### esbuild
```js
import {
  a
} from "./chunk-RLFZNZQZ.js";
export {
  a
};
```
### rolldown
```js
import { b as a } from "./a2.js";

export { a };
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,2 +1,2 @@
-import {a} from "./chunk-RLFZNZQZ.js";
+import {b as a} from "./a2.js";
 export {a};

```
## /out/b.js
### esbuild
```js
import {
  a
} from "./chunk-RLFZNZQZ.js";
export {
  a
};
```
### rolldown
```js
import { b as a } from "./a2.js";

export { a };
```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,2 +1,2 @@
-import {a} from "./chunk-RLFZNZQZ.js";
+import {b as a} from "./a2.js";
 export {a};

```
## /out/chunk-RLFZNZQZ.js
### esbuild
```js
// a.js
var a = 1;

export {
  a
};
```
### rolldown
```js
//#region a.js
const a = 1;

//#endregion
export { a as b };
```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-RLFZNZQZ.js
+++ rolldown	a2.js
@@ -1,2 +1,2 @@
 var a = 1;
-export {a};
+export {a as b};

```