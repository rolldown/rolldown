# Diff
## /out.js
### esbuild
```js
class Derived extends Base {
  test(key) {
    return __async(this, null, function* () {
      var _a, _b, _c, _d;
      return [
        yield __superGet(Derived.prototype, this, "foo"),
        yield __superGet(Derived.prototype, this, key),
        yield [__superWrapper(Derived.prototype, this, "foo")._] = [0],
        yield [__superWrapper(Derived.prototype, this, key)._] = [0],
        yield __superSet(Derived.prototype, this, "foo", 1),
        yield __superSet(Derived.prototype, this, key, 1),
        yield __superSet(Derived.prototype, this, "foo", __superGet(Derived.prototype, this, "foo") + 2),
        yield __superSet(Derived.prototype, this, key, __superGet(Derived.prototype, this, key) + 2),
        yield ++__superWrapper(Derived.prototype, this, "foo")._,
        yield ++__superWrapper(Derived.prototype, this, key)._,
        yield __superWrapper(Derived.prototype, this, "foo")._++,
        yield __superWrapper(Derived.prototype, this, key)._++,
        yield __superGet(Derived.prototype, this, "foo").name,
        yield __superGet(Derived.prototype, this, key).name,
        yield (_a = __superGet(Derived.prototype, this, "foo")) == null ? void 0 : _a.name,
        yield (_b = __superGet(Derived.prototype, this, key)) == null ? void 0 : _b.name,
        yield __superGet(Derived.prototype, this, "foo").call(this, 1, 2),
        yield __superGet(Derived.prototype, this, key).call(this, 1, 2),
        yield (_c = __superGet(Derived.prototype, this, "foo")) == null ? void 0 : _c.call(this, 1, 2),
        yield (_d = __superGet(Derived.prototype, this, key)) == null ? void 0 : _d.call(this, 1, 2),
        yield (() => __superGet(Derived.prototype, this, "foo"))(),
        yield (() => __superGet(Derived.prototype, this, key))(),
        yield (() => __superGet(Derived.prototype, this, "foo").call(this))(),
        yield (() => __superGet(Derived.prototype, this, key).call(this))(),
        yield __superGet(Derived.prototype, this, "foo").bind(this)``,
        yield __superGet(Derived.prototype, this, key).bind(this)``
      ];
    });
  }
}
let fn = () => __async(this, null, function* () {
  return class extends Base {
    constructor() {
      super(...arguments);
      __publicField(this, "a", super.a);
      __publicField(this, "b", () => super.b);
    }
    c() {
      return super.c;
    }
    d() {
      return () => super.d;
    }
  };
});
class Derived2 extends Base {
  constructor() {
    super(...arguments);
    __publicField(this, "b", () => __async(this, null, function* () {
      var _a;
      return _a = __superGet(Derived2.prototype, this, "foo"), class {
        constructor() {
          __publicField(this, _a, 123);
        }
      };
    }));
  }
  a() {
    return __async(this, null, function* () {
      var _a;
      return _a = __superGet(Derived2.prototype, this, "foo"), class {
        constructor() {
          __publicField(this, _a, 123);
        }
      };
    });
  }
}
for (let i = 0; i < 3; i++) {
  let _a;
  objs.push(_a = {
    __proto__: {
      foo() {
        return i;
      }
    },
    bar() {
      return __async(this, null, function* () {
        return __superGet(_a, this, "foo").call(this);
      });
    }
  });
}
```
### rolldown
```js

//#region entry.js
for (let i = 0; i < 3; i++) {
	objs.push({
		__proto__: { foo() {
			return i;
		} },
		async bar() {
			return super.foo();
		}
	});
}

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,61 +1,12 @@
-class Derived extends Base {
-    test(key) {
-        return __async(this, null, function* () {
-            var _a, _b, _c, _d;
-            return [yield __superGet(Derived.prototype, this, "foo"), yield __superGet(Derived.prototype, this, key), yield [__superWrapper(Derived.prototype, this, "foo")._] = [0], yield [__superWrapper(Derived.prototype, this, key)._] = [0], yield __superSet(Derived.prototype, this, "foo", 1), yield __superSet(Derived.prototype, this, key, 1), yield __superSet(Derived.prototype, this, "foo", __superGet(Derived.prototype, this, "foo") + 2), yield __superSet(Derived.prototype, this, key, __superGet(Derived.prototype, this, key) + 2), yield ++__superWrapper(Derived.prototype, this, "foo")._, yield ++__superWrapper(Derived.prototype, this, key)._, yield __superWrapper(Derived.prototype, this, "foo")._++, yield __superWrapper(Derived.prototype, this, key)._++, yield __superGet(Derived.prototype, this, "foo").name, yield __superGet(Derived.prototype, this, key).name, yield (_a = __superGet(Derived.prototype, this, "foo")) == null ? void 0 : _a.name, yield (_b = __superGet(Derived.prototype, this, key)) == null ? void 0 : _b.name, yield __superGet(Derived.prototype, this, "foo").call(this, 1, 2), yield __superGet(Derived.prototype, this, key).call(this, 1, 2), yield (_c = __superGet(Derived.prototype, this, "foo")) == null ? void 0 : _c.call(this, 1, 2), yield (_d = __superGet(Derived.prototype, this, key)) == null ? void 0 : _d.call(this, 1, 2), yield (() => __superGet(Derived.prototype, this, "foo"))(), yield (() => __superGet(Derived.prototype, this, key))(), yield (() => __superGet(Derived.prototype, this, "foo").call(this))(), yield (() => __superGet(Derived.prototype, this, key).call(this))(), yield (__superGet(Derived.prototype, this, "foo").bind(this))``, yield (__superGet(Derived.prototype, this, key).bind(this))``];
-        });
-    }
-}
-let fn = () => __async(this, null, function* () {
-    return class extends Base {
-        constructor() {
-            super(...arguments);
-            __publicField(this, "a", super.a);
-            __publicField(this, "b", () => super.b);
-        }
-        c() {
-            return super.c;
-        }
-        d() {
-            return () => super.d;
-        }
-    };
-});
-class Derived2 extends Base {
-    constructor() {
-        super(...arguments);
-        __publicField(this, "b", () => __async(this, null, function* () {
-            var _a;
-            return (_a = __superGet(Derived2.prototype, this, "foo"), class {
-                constructor() {
-                    __publicField(this, _a, 123);
-                }
-            });
-        }));
-    }
-    a() {
-        return __async(this, null, function* () {
-            var _a;
-            return (_a = __superGet(Derived2.prototype, this, "foo"), class {
-                constructor() {
-                    __publicField(this, _a, 123);
-                }
-            });
-        });
-    }
-}
 for (let i = 0; i < 3; i++) {
-    let _a;
-    objs.push(_a = {
+    objs.push({
         __proto__: {
             foo() {
                 return i;
             }
         },
-        bar() {
-            return __async(this, null, function* () {
-                return __superGet(_a, this, "foo").call(this);
-            });
+        async bar() {
+            return super.foo();
         }
     });
 }

```