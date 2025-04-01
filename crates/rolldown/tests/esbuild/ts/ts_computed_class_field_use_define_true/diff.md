# Reason
1. lowering class
# Diff
## /out.js
### esbuild
```js
var _a, _b;
class Foo {
  [q];
  [r] = s;
  [_b = x];
  [_a = y] = z;
}
__decorateClass([
  dec
], Foo.prototype, _b, 2);
__decorateClass([
  dec
], Foo.prototype, _a, 2);
new Foo();
```
### rolldown
```js

//#region entry.ts
var Foo = class {
	[q];
	[r] = s;
	@dec [x];
	@dec [y] = z;
};
new Foo();
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,10 +1,10 @@
-var _a, _b;
-class Foo {
-    [q];
-    [r] = s;
-    [_b = x];
-    [_a = y] = z;
-}
-__decorateClass([dec], Foo.prototype, _b, 2);
-__decorateClass([dec], Foo.prototype, _a, 2);
+
+//#region entry.ts
+var Foo = class {
+	[q];
+	[r] = s;
+	@dec [x];
+	@dec [y] = z;
+};
 new Foo();
+//#endregion

```