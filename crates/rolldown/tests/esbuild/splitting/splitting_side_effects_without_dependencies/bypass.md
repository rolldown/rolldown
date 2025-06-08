# Reason
1. different chunk naming style
# Diff
## /out/a.js
### esbuild
```js
import {
  a
} from "./chunk-Y3CWGI3W.js";

// a.js
console.log(a);
```
### rolldown
```js
import { b as a } from "./shared.js";

//#region a.js
console.log(a);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,2 +1,2 @@
-import {a} from "./chunk-Y3CWGI3W.js";
+import {b as a} from "./shared.js";
 console.log(a);

```
## /out/b.js
### esbuild
```js
import {
  b
} from "./chunk-Y3CWGI3W.js";

// b.js
console.log(b);
```
### rolldown
```js
import { c as b } from "./shared.js";

//#region b.js
console.log(b);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,2 +1,2 @@
-import {b} from "./chunk-Y3CWGI3W.js";
+import {c as b} from "./shared.js";
 console.log(b);

```
## /out/chunk-Y3CWGI3W.js
### esbuild
```js
// shared.js
var a = 1;
var b = 2;
console.log("side effect");

export {
  a,
  b
};
```
### rolldown
```js
//#region shared.js
let a = 1;
let b = 2;
console.log("side effect");

//#endregion
export { a as b, b as c };
```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-Y3CWGI3W.js
+++ rolldown	shared.js
@@ -1,4 +1,4 @@
 var a = 1;
 var b = 2;
 console.log("side effect");
-export {a, b};
+export {a as b, b as c};

```