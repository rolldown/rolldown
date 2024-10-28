# Diff
## /out.js
### esbuild
```js
const keepThisToo = Symbol("keepThisToo");
class Foo {
  keepThis;
  [keepThisToo];
}
(() => new Foo())();
```
### rolldown
```js

//#region entry.ts
const keepThisToo = Symbol("keepThisToo");
var Foo = class {
	keepThis;
	[keepThisToo];
};
(() => new Foo())();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,6 @@
 const keepThisToo = Symbol("keepThisToo");
-class Foo {
+var Foo = class {
     keepThis;
     [keepThisToo];
-}
+};
 (() => new Foo())();

```