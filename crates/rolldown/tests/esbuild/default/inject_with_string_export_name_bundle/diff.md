# Diff
## /out.js
### esbuild
```js
// inject.js
var old = console.log;
var fn = (...args) => old.apply(console, ["log:"].concat(args));

// entry.js
fn(test);
fn(test);
fn(test);
```
### rolldown
```js

//#region entry.js
console.log(test);
console.info(test);
console.warn(test);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,3 @@
-var old = console.log;
-var fn = (...args) => old.apply(console, ["log:"].concat(args));
-fn(test);
-fn(test);
-fn(test);
+console.log(test);
+console.info(test);
+console.warn(test);

```