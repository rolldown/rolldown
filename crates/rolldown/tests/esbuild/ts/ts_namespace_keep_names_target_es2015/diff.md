# Reason
1. needs support target
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
  const _Baz = class _Baz {
  };
  __name(_Baz, "Baz");
  let Baz = _Baz;
  ns2.Baz = _Baz;
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
@@ -1,11 +1,8 @@
 var ns;
-(ns2 => {
-    ns2.foo = __name(() => {}, "foo");
+(function (_ns) {
+    _ns.foo = () => {};
     function bar() {}
-    ns2.bar = bar;
-    __name(bar, "bar");
-    const _Baz = class _Baz {};
-    __name(_Baz, "Baz");
-    let Baz = _Baz;
-    ns2.Baz = _Baz;
+    _ns.bar = bar;
+    class Baz {}
+    _ns.Baz = Baz;
 })(ns || (ns = {}));

```