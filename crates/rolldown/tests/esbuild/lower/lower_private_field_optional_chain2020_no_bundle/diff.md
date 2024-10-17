# Diff
## /out.js
### esbuild
```js
var _x;
class Foo {
  constructor() {
    __privateAdd(this, _x);
  }
  foo() {
    this == null ? void 0 : __privateGet(this, _x).y;
    this == null ? void 0 : __privateGet(this.y, _x);
    __privateGet(this, _x)?.y;
  }
}
_x = new WeakMap();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,12 +0,0 @@
-var _x;
-class Foo {
-    constructor() {
-        __privateAdd(this, _x);
-    }
-    foo() {
-        this == null ? void 0 : __privateGet(this, _x).y;
-        this == null ? void 0 : __privateGet(this.y, _x);
-        __privateGet(this, _x)?.y;
-    }
-}
-_x = new WeakMap();

```