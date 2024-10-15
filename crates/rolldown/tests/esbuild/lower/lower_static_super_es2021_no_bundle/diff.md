# Diff
## /out.js
### esbuild
```js
const _Derived = class _Derived extends Base {
};
__publicField(_Derived, "test", (key) => {
  return [
    __superGet(_Derived, _Derived, "foo"),
    __superGet(_Derived, _Derived, key),
    [__superWrapper(_Derived, _Derived, "foo")._] = [0],
    [__superWrapper(_Derived, _Derived, key)._] = [0],
    __superSet(_Derived, _Derived, "foo", 1),
    __superSet(_Derived, _Derived, key, 1),
    __superSet(_Derived, _Derived, "foo", __superGet(_Derived, _Derived, "foo") + 2),
    __superSet(_Derived, _Derived, key, __superGet(_Derived, _Derived, key) + 2),
    ++__superWrapper(_Derived, _Derived, "foo")._,
    ++__superWrapper(_Derived, _Derived, key)._,
    __superWrapper(_Derived, _Derived, "foo")._++,
    __superWrapper(_Derived, _Derived, key)._++,
    __superGet(_Derived, _Derived, "foo").name,
    __superGet(_Derived, _Derived, key).name,
    __superGet(_Derived, _Derived, "foo")?.name,
    __superGet(_Derived, _Derived, key)?.name,
    __superGet(_Derived, _Derived, "foo").call(this, 1, 2),
    __superGet(_Derived, _Derived, key).call(this, 1, 2),
    super.foo?.(1, 2),
    super[key]?.(1, 2),
    (() => __superGet(_Derived, _Derived, "foo"))(),
    (() => __superGet(_Derived, _Derived, key))(),
    (() => __superGet(_Derived, _Derived, "foo").call(this))(),
    (() => __superGet(_Derived, _Derived, key).call(this))(),
    __superGet(_Derived, _Derived, "foo").bind(this)``,
    __superGet(_Derived, _Derived, key).bind(this)``
  ];
});
let Derived = _Derived;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,33 +0,0 @@
-const _Derived = class _Derived extends Base {
-};
-__publicField(_Derived, "test", (key) => {
-  return [
-    __superGet(_Derived, _Derived, "foo"),
-    __superGet(_Derived, _Derived, key),
-    [__superWrapper(_Derived, _Derived, "foo")._] = [0],
-    [__superWrapper(_Derived, _Derived, key)._] = [0],
-    __superSet(_Derived, _Derived, "foo", 1),
-    __superSet(_Derived, _Derived, key, 1),
-    __superSet(_Derived, _Derived, "foo", __superGet(_Derived, _Derived, "foo") + 2),
-    __superSet(_Derived, _Derived, key, __superGet(_Derived, _Derived, key) + 2),
-    ++__superWrapper(_Derived, _Derived, "foo")._,
-    ++__superWrapper(_Derived, _Derived, key)._,
-    __superWrapper(_Derived, _Derived, "foo")._++,
-    __superWrapper(_Derived, _Derived, key)._++,
-    __superGet(_Derived, _Derived, "foo").name,
-    __superGet(_Derived, _Derived, key).name,
-    __superGet(_Derived, _Derived, "foo")?.name,
-    __superGet(_Derived, _Derived, key)?.name,
-    __superGet(_Derived, _Derived, "foo").call(this, 1, 2),
-    __superGet(_Derived, _Derived, key).call(this, 1, 2),
-    super.foo?.(1, 2),
-    super[key]?.(1, 2),
-    (() => __superGet(_Derived, _Derived, "foo"))(),
-    (() => __superGet(_Derived, _Derived, key))(),
-    (() => __superGet(_Derived, _Derived, "foo").call(this))(),
-    (() => __superGet(_Derived, _Derived, key).call(this))(),
-    __superGet(_Derived, _Derived, "foo").bind(this)``,
-    __superGet(_Derived, _Derived, key).bind(this)``
-  ];
-});
-let Derived = _Derived;
\ No newline at end of file

```