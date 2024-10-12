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
class Foo {
	[q];
	[r] = s;
	@dec [x];
	@dec [y] = z;
}
new Foo();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,10 +1,11 @@
-var _a, _b;
+
+//#region entry.ts
 class Foo {
-    [q];
-    [r] = s;
-    [_b = x];
-    [_a = y] = z;
+	[q];
+	[r] = s;
+	@dec [x];
+	@dec [y] = z;
 }
-__decorateClass([dec], Foo.prototype, _b, 2);
-__decorateClass([dec], Foo.prototype, _a, 2);
 new Foo();
+
+//#endregion

```