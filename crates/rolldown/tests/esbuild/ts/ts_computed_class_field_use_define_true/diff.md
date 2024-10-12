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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,10 +0,0 @@
-var _a, _b;
-class Foo {
-    [q];
-    [r] = s;
-    [_b = x];
-    [_a = y] = z;
-}
-__decorateClass([dec], Foo.prototype, _b, 2);
-__decorateClass([dec], Foo.prototype, _a, 2);
-new Foo();

```