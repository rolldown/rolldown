# Diff
## /out.js
### esbuild
```js
var _e;
export class A {
}
export class B extends A {
  constructor(c) {
    var _a;
    super();
    __privateAdd(this, _e);
    __privateSet(this, _e, (_a = c.d) != null ? _a : "test");
  }
  f() {
    return __privateGet(this, _e);
  }
}
_e = new WeakMap();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,14 +0,0 @@
-var _e;
-export class A {}
-export class B extends A {
-    constructor(c) {
-        var _a;
-        super();
-        __privateAdd(this, _e);
-        __privateSet(this, _e, (_a = c.d) != null ? _a : "test");
-    }
-    f() {
-        return __privateGet(this, _e);
-    }
-}
-_e = new WeakMap();

```