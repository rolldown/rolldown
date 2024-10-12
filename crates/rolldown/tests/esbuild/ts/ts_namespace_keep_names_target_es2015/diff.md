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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,11 +0,0 @@
-var ns;
-(ns2 => {
-    ns2.foo = __name(() => {}, "foo");
-    function bar() {}
-    ns2.bar = bar;
-    __name(bar, "bar");
-    const _Baz = class _Baz {};
-    __name(_Baz, "Baz");
-    let Baz = _Baz;
-    ns2.Baz = _Baz;
-})(ns || (ns = {}));

```