# Reason
1. could be done in minifier
2. we don't have plan to support keep_names in rolldown
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
//#region entry.ts
let ns;
(function(_ns) {
	_ns.foo = () => {};
	function bar() {}
	_ns.bar = bar;
	class Baz {}
	_ns.Baz = Baz;
})(ns || (ns = {}));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,13 +1,8 @@
 var ns;
-(ns2 => {
-    ns2.foo = __name(() => {}, "foo");
+(function (_ns) {
+    _ns.foo = () => {};
     function bar() {}
-    ns2.bar = bar;
-    __name(bar, "bar");
-    class Baz {
-        static {
-            __name(this, "Baz");
-        }
-    }
-    ns2.Baz = Baz;
+    _ns.bar = bar;
+    class Baz {}
+    _ns.Baz = Baz;
 })(ns || (ns = {}));

```