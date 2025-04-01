# Reason
1. lowering decorator
# Diff
## /out.js
### esbuild
```js
let foo = 1;
class Foo {
  method1(foo2 = 2) {
  }
  method2(foo2 = 3) {
  }
}
__decorateClass([
  __decorateParam(0, dec(foo))
], Foo.prototype, "method1", 1);
__decorateClass([
  __decorateParam(0, dec(() => foo))
], Foo.prototype, "method2", 1);
class Bar {
  static {
    this.x = class {
      static {
        this.y = () => {
          let bar = 1;
          let Baz = class {
            method1() {
            }
            method2() {
            }
            method3(bar2) {
            }
            method4(bar2) {
            }
          };
          __decorateClass([
            dec(bar)
          ], Baz.prototype, "method1", 1);
          __decorateClass([
            dec(() => bar)
          ], Baz.prototype, "method2", 1);
          __decorateClass([
            __decorateParam(0, dec(() => bar))
          ], Baz.prototype, "method3", 1);
          __decorateClass([
            __decorateParam(0, dec(() => bar))
          ], Baz.prototype, "method4", 1);
          Baz = __decorateClass([
            dec(bar),
            dec(() => bar)
          ], Baz);
          return Baz;
        };
      }
    };
  }
}
```
### rolldown
```js

//#region entry.ts
var Foo = class {
	method1(@dec(foo) foo = 2) {}
	method2(@dec(() => foo) foo = 3) {}
};
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,30 +1,7 @@
-let foo = 1;
-class Foo {
-    method1(foo2 = 2) {}
-    method2(foo2 = 3) {}
-}
-__decorateClass([__decorateParam(0, dec(foo))], Foo.prototype, "method1", 1);
-__decorateClass([__decorateParam(0, dec(() => foo))], Foo.prototype, "method2", 1);
-class Bar {
-    static {
-        this.x = class {
-            static {
-                this.y = () => {
-                    let bar = 1;
-                    let Baz = class {
-                        method1() {}
-                        method2() {}
-                        method3(bar2) {}
-                        method4(bar2) {}
-                    };
-                    __decorateClass([dec(bar)], Baz.prototype, "method1", 1);
-                    __decorateClass([dec(() => bar)], Baz.prototype, "method2", 1);
-                    __decorateClass([__decorateParam(0, dec(() => bar))], Baz.prototype, "method3", 1);
-                    __decorateClass([__decorateParam(0, dec(() => bar))], Baz.prototype, "method4", 1);
-                    Baz = __decorateClass([dec(bar), dec(() => bar)], Baz);
-                    return Baz;
-                };
-            }
-        };
-    }
-}
+
+//#region entry.ts
+var Foo = class {
+	method1(@dec(foo) foo = 2) {}
+	method2(@dec(() => foo) foo = 3) {}
+};
+//#endregion

```