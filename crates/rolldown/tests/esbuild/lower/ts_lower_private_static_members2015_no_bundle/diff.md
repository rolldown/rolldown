# Diff
## /out.js
### esbuild
```js
var _x, _Foo_static, y_get, y_set, z_fn;
const _Foo = class _Foo {
  foo() {
    var _a;
    __privateSet(_Foo, _x, __privateGet(_Foo, _x) + 1);
    __privateSet(_Foo, _Foo_static, __privateGet(_Foo, _Foo_static, y_get) + 1, y_set);
    __privateMethod(_a = _Foo, _Foo_static, z_fn).call(_a);
  }
};
_x = new WeakMap();
_Foo_static = new WeakSet();
y_get = function() {
};
y_set = function(x) {
};
z_fn = function() {
};
__privateAdd(_Foo, _Foo_static);
__privateAdd(_Foo, _x);
let Foo = _Foo;
```
### rolldown
```js


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,17 +0,0 @@
-var _x, _Foo_static, y_get, y_set, z_fn;
-const _Foo = class _Foo {
-    foo() {
-        var _a;
-        __privateSet(_Foo, _x, __privateGet(_Foo, _x) + 1);
-        __privateSet(_Foo, _Foo_static, __privateGet(_Foo, _Foo_static, y_get) + 1, y_set);
-        __privateMethod(_a = _Foo, _Foo_static, z_fn).call(_a);
-    }
-};
-_x = new WeakMap();
-_Foo_static = new WeakSet();
-y_get = function () {};
-y_set = function (x) {};
-z_fn = function () {};
-__privateAdd(_Foo, _Foo_static);
-__privateAdd(_Foo, _x);
-let Foo = _Foo;

```