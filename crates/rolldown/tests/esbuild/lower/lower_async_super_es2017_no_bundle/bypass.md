# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out.js
### esbuild
```js
class Derived extends Base {
  async test(key) {
    var _a, _b, _c, _d;
    return [
      await super.foo,
      await super[key],
      await ([super.foo] = [0]),
      await ([super[key]] = [0]),
      await (super.foo = 1),
      await (super[key] = 1),
      await (super.foo += 2),
      await (super[key] += 2),
      await ++super.foo,
      await ++super[key],
      await super.foo++,
      await super[key]++,
      await super.foo.name,
      await super[key].name,
      await ((_a = super.foo) == null ? void 0 : _a.name),
      await ((_b = super[key]) == null ? void 0 : _b.name),
      await super.foo(1, 2),
      await super[key](1, 2),
      await ((_c = super.foo) == null ? void 0 : _c.call(this, 1, 2)),
      await ((_d = super[key]) == null ? void 0 : _d.call(this, 1, 2)),
      await (() => super.foo)(),
      await (() => super[key])(),
      await (() => super.foo())(),
      await (() => super[key]())(),
      await super.foo``,
      await super[key]``
    ];
  }
}
let fn = async () => class extends Base {
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
class Derived2 extends Base {
  constructor() {
    super(...arguments);
    __publicField(this, "b", async () => {
      var _a;
      return _a = super.foo, class {
        constructor() {
          __publicField(this, _a, 123);
        }
      };
    });
  }
  async a() {
    var _a;
    return _a = super.foo, class {
      constructor() {
        __publicField(this, _a, 123);
      }
    };
  }
}
for (let i = 0; i < 3; i++) {
  objs.push({
    __proto__: {
      foo() {
        return i;
      }
    },
    async bar() {
      return super.foo();
    }
  });
}
```
### rolldown
```js
//#region entry.js
// This covers putting the generated temporary variable inside the loop
for (let i = 0; i < 3; i++) objs.push({
	__proto__: { foo() {
		return i;
	} },
	async bar() {
		return super.foo();
	}
});

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,52 +1,10 @@
-class Derived extends Base {
-    async test(key) {
-        var _a, _b, _c, _d;
-        return [await super.foo, await super[key], await ([super.foo] = [0]), await ([super[key]] = [0]), await (super.foo = 1), await (super[key] = 1), await (super.foo += 2), await (super[key] += 2), await ++super.foo, await ++super[key], await super.foo++, await super[key]++, await super.foo.name, await super[key].name, await ((_a = super.foo) == null ? void 0 : _a.name), await ((_b = super[key]) == null ? void 0 : _b.name), await super.foo(1, 2), await super[key](1, 2), await ((_c = super.foo) == null ? void 0 : _c.call(this, 1, 2)), await ((_d = super[key]) == null ? void 0 : _d.call(this, 1, 2)), await (() => super.foo)(), await (() => super[key])(), await (() => super.foo())(), await (() => super[key]())(), await (super.foo)``, await (super[key])``];
-    }
-}
-let fn = async () => class extends Base {
-    constructor() {
-        super(...arguments);
-        __publicField(this, "a", super.a);
-        __publicField(this, "b", () => super.b);
-    }
-    c() {
-        return super.c;
-    }
-    d() {
-        return () => super.d;
-    }
-};
-class Derived2 extends Base {
-    constructor() {
-        super(...arguments);
-        __publicField(this, "b", async () => {
-            var _a;
-            return (_a = super.foo, class {
-                constructor() {
-                    __publicField(this, _a, 123);
-                }
-            });
-        });
-    }
-    async a() {
-        var _a;
-        return (_a = super.foo, class {
-            constructor() {
-                __publicField(this, _a, 123);
-            }
-        });
-    }
-}
-for (let i = 0; i < 3; i++) {
-    objs.push({
-        __proto__: {
-            foo() {
-                return i;
-            }
-        },
-        async bar() {
-            return super.foo();
+for (let i = 0; i < 3; i++) objs.push({
+    __proto__: {
+        foo() {
+            return i;
         }
-    });
-}
+    },
+    async bar() {
+        return super.foo();
+    }
+});

```