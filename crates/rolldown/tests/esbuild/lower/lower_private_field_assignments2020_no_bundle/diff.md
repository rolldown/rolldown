# Diff
## /out.js
### esbuild
```js
var _x;
class Foo {
  constructor() {
    __privateAdd(this, _x);
  }
  unary() {
    __privateWrapper(this, _x)._++;
    __privateWrapper(this, _x)._--;
    ++__privateWrapper(this, _x)._;
    --__privateWrapper(this, _x)._;
  }
  binary() {
    __privateSet(this, _x, 1);
    __privateSet(this, _x, __privateGet(this, _x) + 1);
    __privateSet(this, _x, __privateGet(this, _x) - 1);
    __privateSet(this, _x, __privateGet(this, _x) * 1);
    __privateSet(this, _x, __privateGet(this, _x) / 1);
    __privateSet(this, _x, __privateGet(this, _x) % 1);
    __privateSet(this, _x, __privateGet(this, _x) ** 1);
    __privateSet(this, _x, __privateGet(this, _x) << 1);
    __privateSet(this, _x, __privateGet(this, _x) >> 1);
    __privateSet(this, _x, __privateGet(this, _x) >>> 1);
    __privateSet(this, _x, __privateGet(this, _x) & 1);
    __privateSet(this, _x, __privateGet(this, _x) | 1);
    __privateSet(this, _x, __privateGet(this, _x) ^ 1);
    __privateGet(this, _x) && __privateSet(this, _x, 1);
    __privateGet(this, _x) || __privateSet(this, _x, 1);
    __privateGet(this, _x) ?? __privateSet(this, _x, 1);
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
@@ -1,31 +0,0 @@
-var _x;
-class Foo {
-    constructor() {
-        __privateAdd(this, _x);
-    }
-    unary() {
-        __privateWrapper(this, _x)._++;
-        __privateWrapper(this, _x)._--;
-        ++__privateWrapper(this, _x)._;
-        --__privateWrapper(this, _x)._;
-    }
-    binary() {
-        __privateSet(this, _x, 1);
-        __privateSet(this, _x, __privateGet(this, _x) + 1);
-        __privateSet(this, _x, __privateGet(this, _x) - 1);
-        __privateSet(this, _x, __privateGet(this, _x) * 1);
-        __privateSet(this, _x, __privateGet(this, _x) / 1);
-        __privateSet(this, _x, __privateGet(this, _x) % 1);
-        __privateSet(this, _x, __privateGet(this, _x) ** 1);
-        __privateSet(this, _x, __privateGet(this, _x) << 1);
-        __privateSet(this, _x, __privateGet(this, _x) >> 1);
-        __privateSet(this, _x, __privateGet(this, _x) >>> 1);
-        __privateSet(this, _x, __privateGet(this, _x) & 1);
-        __privateSet(this, _x, __privateGet(this, _x) | 1);
-        __privateSet(this, _x, __privateGet(this, _x) ^ 1);
-        __privateGet(this, _x) && __privateSet(this, _x, 1);
-        __privateGet(this, _x) || __privateSet(this, _x, 1);
-        __privateGet(this, _x) ?? __privateSet(this, _x, 1);
-    }
-}
-_x = new WeakMap();

```