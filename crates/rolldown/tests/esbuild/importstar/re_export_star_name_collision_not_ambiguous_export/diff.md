## /out.js
### esbuild
```js
// c.js
var x = 1;
var y = 2;
export {
  x,
  y
};
```
### rolldown
```js

//#region c.js
let x = 1;
let y = 2;

//#endregion
export { x, y };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.mjs
@@ -1,6 +1,6 @@
-var x = 1;
-var y = 2;
+let x = 1;
+let y = 2;
 export {
     x,
     y
 };
\ No newline at end of file

```
