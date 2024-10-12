# Diff
## /out.js
### esbuild
```js
var old = console.log;
var fn = (...args) => old.apply(console, ["log:"].concat(args));
fn(test);
```
### rolldown
```js

//#region entry.js
console.log(test);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,3 +1,1 @@
-var old = console.log;
-var fn = (...args) => old.apply(console, ["log:"].concat(args));
-fn(test);
+console.log(test);

```