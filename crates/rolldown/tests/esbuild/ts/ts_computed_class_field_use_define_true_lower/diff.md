# Diff
## /out.js
### esbuild
```js
var _a, _b, _c, _d;
_d = q, _c = r, _b = x, _a = y;
class Foo {
  constructor() {
    __publicField(this, _d);
    __publicField(this, _c, s);
    __publicField(this, _b);
    __publicField(this, _a, z);
  }
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
@@ -1,13 +0,0 @@
-var _a, _b, _c, _d;
-(_d = q, _c = r, _b = x, _a = y);
-class Foo {
-    constructor() {
-        __publicField(this, _d);
-        __publicField(this, _c, s);
-        __publicField(this, _b);
-        __publicField(this, _a, z);
-    }
-}
-__decorateClass([dec], Foo.prototype, _b, 2);
-__decorateClass([dec], Foo.prototype, _a, 2);
-new Foo();

```