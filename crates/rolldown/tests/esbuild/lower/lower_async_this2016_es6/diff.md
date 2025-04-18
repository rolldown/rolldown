# Diff
## /out.js
### esbuild
```js
// other.js
var bar = () => __async(void 0, null, function* () {
});

// entry.js
var foo = () => __async(void 0, null, function* () {
  return void 0;
});
export {
  bar,
  foo
};
```
### rolldown
```js
//#region other.js
let bar = async () => {};

//#endregion
//#region entry.js
let foo = async () => void 0;

//#endregion
export { bar, foo };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,3 @@
-var bar = () => __async(void 0, null, function* () {});
-var foo = () => __async(void 0, null, function* () {
-    return void 0;
-});
+var bar = async () => {};
+var foo = async () => void 0;
 export {bar, foo};

```