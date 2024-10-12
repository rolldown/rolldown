# Diff
## /out.js
### esbuild
```js
const _Derived = class _Derived extends Base {
};
__publicField(_Derived, "test", (key) => {
  var _a, _b, _c, _d;
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
    (_a = __superGet(_Derived, _Derived, "foo")) == null ? void 0 : _a.name,
    (_b = __superGet(_Derived, _Derived, key)) == null ? void 0 : _b.name,
    __superGet(_Derived, _Derived, "foo").call(this, 1, 2),
    __superGet(_Derived, _Derived, key).call(this, 1, 2),
    (_c = __superGet(_Derived, _Derived, "foo")) == null ? void 0 : _c.call(this, 1, 2),
    (_d = __superGet(_Derived, _Derived, key)) == null ? void 0 : _d.call(this, 1, 2),
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
@@ -1,6 +0,0 @@
-const _Derived = class _Derived extends Base {};
-__publicField(_Derived, "test", key => {
-    var _a, _b, _c, _d;
-    return [__superGet(_Derived, _Derived, "foo"), __superGet(_Derived, _Derived, key), [__superWrapper(_Derived, _Derived, "foo")._] = [0], [__superWrapper(_Derived, _Derived, key)._] = [0], __superSet(_Derived, _Derived, "foo", 1), __superSet(_Derived, _Derived, key, 1), __superSet(_Derived, _Derived, "foo", __superGet(_Derived, _Derived, "foo") + 2), __superSet(_Derived, _Derived, key, __superGet(_Derived, _Derived, key) + 2), ++__superWrapper(_Derived, _Derived, "foo")._, ++__superWrapper(_Derived, _Derived, key)._, __superWrapper(_Derived, _Derived, "foo")._++, __superWrapper(_Derived, _Derived, key)._++, __superGet(_Derived, _Derived, "foo").name, __superGet(_Derived, _Derived, key).name, (_a = __superGet(_Derived, _Derived, "foo")) == null ? void 0 : _a.name, (_b = __superGet(_Derived, _Derived, key)) == null ? void 0 : _b.name, __superGet(_Derived, _Derived, "foo").call(this, 1, 2), __superGet(_Derived, _Derived, key).call(this, 1, 2), (_c = __superGet(_Derived, _Derived, "foo")) == null ? void 0 : _c.call(this, 1, 2), (_d = __superGet(_Derived, _Derived, key)) == null ? void 0 : _d.call(this, 1, 2), (() => __superGet(_Derived, _Derived, "foo"))(), (() => __superGet(_Derived, _Derived, key))(), (() => __superGet(_Derived, _Derived, "foo").call(this))(), (() => __superGet(_Derived, _Derived, key).call(this))(), (__superGet(_Derived, _Derived, "foo").bind(this))``, (__superGet(_Derived, _Derived, key).bind(this))``];
-});
-let Derived = _Derived;

```