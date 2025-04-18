# Reason
1. replace the function it self in `inject files`, this align with `@rollup/plugin-inject`
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
//#region inject.js
const old = fn;
const fn = (...args) => old.apply(console, ["log:"].concat(args));

//#endregion
//#region entry.js
fn(test);
fn(test);
fn(test);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,5 @@
-var old = console.log;
+var old = fn;
 var fn = (...args) => old.apply(console, ["log:"].concat(args));
 fn(test);
 fn(test);
 fn(test);

```