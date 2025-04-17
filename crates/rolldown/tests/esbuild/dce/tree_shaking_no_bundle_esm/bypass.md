# Reason
1. we don't have no bundle mode, output should be same if in bundle mode
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