# Diff
## /out.js
### esbuild
```js
function keep() {
}
function unused() {
}
keep();
```
### rolldown
```js

//#region entry.js
function keep() {}
keep();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,2 @@
 function keep() {}
-function unused() {}
 keep();

```