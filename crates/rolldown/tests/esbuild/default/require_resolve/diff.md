# Diff
## /out.js
### esbuild
```js
// entry.js
console.log(require.resolve);
console.log(require.resolve());
console.log(require.resolve(foo));
console.log(require.resolve("a", "b"));
console.log(require.resolve("./present-file"));
console.log(require.resolve("./missing-file"));
console.log(require.resolve("./external-file"));
console.log(require.resolve("missing-pkg"));
console.log(require.resolve("external-pkg"));
console.log(require.resolve("@scope/missing-pkg"));
console.log(require.resolve("@scope/external-pkg"));
try {
  console.log(require.resolve("inside-try"));
} catch (e) {
}
if (false) {
  console.log(null);
}
console.log(false ? null : 0);
console.log(true ? 0 : null);
console.log(false);
console.log(true);
console.log(true);
```
### rolldown
```js

//#region entry.js
console.log(require.resolve);
console.log(require.resolve());
console.log(require.resolve(foo));
console.log(require.resolve("a", "b"));
console.log(require.resolve("./present-file"));
console.log(require.resolve("./missing-file"));
console.log(require.resolve("./external-file"));
console.log(require.resolve("missing-pkg"));
console.log(require.resolve("external-pkg"));
console.log(require.resolve("@scope/missing-pkg"));
console.log(require.resolve("@scope/external-pkg"));
try {
	console.log(require.resolve("inside-try"));
} catch (e) {}
console.log(0);
console.log(0);
console.log(false);
console.log(true);
console.log(true);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -11,12 +11,9 @@
 console.log(require.resolve("@scope/external-pkg"));
 try {
     console.log(require.resolve("inside-try"));
 } catch (e) {}
-if (false) {
-    console.log(null);
-}
-console.log(false ? null : 0);
-console.log(true ? 0 : null);
+console.log(0);
+console.log(0);
 console.log(false);
 console.log(true);
 console.log(true);

```