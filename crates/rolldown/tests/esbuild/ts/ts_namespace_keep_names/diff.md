# Diff
## /out.js
### esbuild
```js
// entry.ts
var ns;
((ns2) => {
  ns2.foo = /* @__PURE__ */ __name(() => {
  }, "foo");
  function bar() {
  }
  ns2.bar = bar;
  __name(bar, "bar");
  class Baz {
    static {
      __name(this, "Baz");
    }
  }
  ns2.Baz = Baz;
})(ns || (ns = {}));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,13 +0,0 @@
-var ns;
-(ns2 => {
-    ns2.foo = __name(() => {}, "foo");
-    function bar() {}
-    ns2.bar = bar;
-    __name(bar, "bar");
-    class Baz {
-        static {
-            __name(this, "Baz");
-        }
-    }
-    ns2.Baz = Baz;
-})(ns || (ns = {}));

```