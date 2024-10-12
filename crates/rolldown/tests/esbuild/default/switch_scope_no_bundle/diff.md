# Diff
## /out.js
### esbuild
```js
switch (foo) {
  default:
    var foo;
}
switch (bar) {
  default:
    let a;
}
```
### rolldown
```js

//#region entry.js
switch (bar) {
	default: let bar$1;
}

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,4 @@
-switch (foo) {
-    default:
-        var foo;
-}
 switch (bar) {
     default:
-        let a;
+        let bar$1;
 }

```