# Diff
## /out.js
### esbuild
```js
const _Derived = class _Derived extends Base {
};
__publicField(_Derived, "test", async (key) => {
  return [
    await __superGet(_Derived, _Derived, "foo"),
    await __superGet(_Derived, _Derived, key),
    await ([__superWrapper(_Derived, _Derived, "foo")._] = [0]),
    await ([__superWrapper(_Derived, _Derived, key)._] = [0]),
    await __superSet(_Derived, _Derived, "foo", 1),
    await __superSet(_Derived, _Derived, key, 1),
    await __superSet(_Derived, _Derived, "foo", __superGet(_Derived, _Derived, "foo") + 2),
    await __superSet(_Derived, _Derived, key, __superGet(_Derived, _Derived, key) + 2),
    await ++__superWrapper(_Derived, _Derived, "foo")._,
    await ++__superWrapper(_Derived, _Derived, key)._,
    await __superWrapper(_Derived, _Derived, "foo")._++,
    await __superWrapper(_Derived, _Derived, key)._++,
    await __superGet(_Derived, _Derived, "foo").name,
    await __superGet(_Derived, _Derived, key).name,
    await __superGet(_Derived, _Derived, "foo")?.name,
    await __superGet(_Derived, _Derived, key)?.name,
    await __superGet(_Derived, _Derived, "foo").call(this, 1, 2),
    await __superGet(_Derived, _Derived, key).call(this, 1, 2),
    await super.foo?.(1, 2),
    await super[key]?.(1, 2),
    await (() => __superGet(_Derived, _Derived, "foo"))(),
    await (() => __superGet(_Derived, _Derived, key))(),
    await (() => __superGet(_Derived, _Derived, "foo").call(this))(),
    await (() => __superGet(_Derived, _Derived, key).call(this))(),
    await __superGet(_Derived, _Derived, "foo").bind(this)``,
    await __superGet(_Derived, _Derived, key).bind(this)``
  ];
});
let Derived = _Derived;
let fn = async () => {
  var _a;
  return _a = class extends Base {
    static c() {
      return super.c;
    }
    static d() {
      return () => super.d;
    }
  }, __publicField(_a, "a", __superGet(_a, _a, "a")), __publicField(_a, "b", () => __superGet(_a, _a, "b")), _a;
};
const _Derived2 = class _Derived2 extends Base {
  static async a() {
    var _a;
    return _a = super.foo, class {
      constructor() {
        __publicField(this, _a, 123);
      }
    };
  }
};
__publicField(_Derived2, "b", async () => {
  var _a;
  return _a = __superGet(_Derived2, _Derived2, "foo"), class {
    constructor() {
      __publicField(this, _a, 123);
    }
  };
});
let Derived2 = _Derived2;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,63 +0,0 @@
-const _Derived = class _Derived extends Base {
-};
-__publicField(_Derived, "test", async (key) => {
-  return [
-    await __superGet(_Derived, _Derived, "foo"),
-    await __superGet(_Derived, _Derived, key),
-    await ([__superWrapper(_Derived, _Derived, "foo")._] = [0]),
-    await ([__superWrapper(_Derived, _Derived, key)._] = [0]),
-    await __superSet(_Derived, _Derived, "foo", 1),
-    await __superSet(_Derived, _Derived, key, 1),
-    await __superSet(_Derived, _Derived, "foo", __superGet(_Derived, _Derived, "foo") + 2),
-    await __superSet(_Derived, _Derived, key, __superGet(_Derived, _Derived, key) + 2),
-    await ++__superWrapper(_Derived, _Derived, "foo")._,
-    await ++__superWrapper(_Derived, _Derived, key)._,
-    await __superWrapper(_Derived, _Derived, "foo")._++,
-    await __superWrapper(_Derived, _Derived, key)._++,
-    await __superGet(_Derived, _Derived, "foo").name,
-    await __superGet(_Derived, _Derived, key).name,
-    await __superGet(_Derived, _Derived, "foo")?.name,
-    await __superGet(_Derived, _Derived, key)?.name,
-    await __superGet(_Derived, _Derived, "foo").call(this, 1, 2),
-    await __superGet(_Derived, _Derived, key).call(this, 1, 2),
-    await super.foo?.(1, 2),
-    await super[key]?.(1, 2),
-    await (() => __superGet(_Derived, _Derived, "foo"))(),
-    await (() => __superGet(_Derived, _Derived, key))(),
-    await (() => __superGet(_Derived, _Derived, "foo").call(this))(),
-    await (() => __superGet(_Derived, _Derived, key).call(this))(),
-    await __superGet(_Derived, _Derived, "foo").bind(this)``,
-    await __superGet(_Derived, _Derived, key).bind(this)``
-  ];
-});
-let Derived = _Derived;
-let fn = async () => {
-  var _a;
-  return _a = class extends Base {
-    static c() {
-      return super.c;
-    }
-    static d() {
-      return () => super.d;
-    }
-  }, __publicField(_a, "a", __superGet(_a, _a, "a")), __publicField(_a, "b", () => __superGet(_a, _a, "b")), _a;
-};
-const _Derived2 = class _Derived2 extends Base {
-  static async a() {
-    var _a;
-    return _a = super.foo, class {
-      constructor() {
-        __publicField(this, _a, 123);
-      }
-    };
-  }
-};
-__publicField(_Derived2, "b", async () => {
-  var _a;
-  return _a = __superGet(_Derived2, _Derived2, "foo"), class {
-    constructor() {
-      __publicField(this, _a, 123);
-    }
-  };
-});
-let Derived2 = _Derived2;
\ No newline at end of file

```