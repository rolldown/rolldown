# Diff
## /out.js
### esbuild
```js
const keepThis = Symbol("keepThis");
keepThis;
class Foo {
}
(() => new Foo())();
```
### rolldown
```js

//#region entry.ts
const keepThis = Symbol("keepThis");
class Foo {
	REMOVE_THIS;
	[keepThis];
}
(() => new Foo())();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,4 +1,6 @@
-const keepThis = Symbol("keepThis");
-keepThis;
-class Foo {}
+var keepThis = Symbol("keepThis");
+class Foo {
+    REMOVE_THIS;
+    [keepThis];
+}
 (() => new Foo())();

```