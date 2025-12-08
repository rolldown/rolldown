## /out.js
### esbuild
```js
// foo.ts
var require_foo = __commonJS({
  "foo.ts"(exports) {
    exports.foo = 123;
  }
});

// entry.ts
var ns = __toESM(require_foo());
var foo2 = 234;
console.log(ns.foo, ns.foo, foo2);
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region foo.ts
var require_foo = /* @__PURE__ */ __commonJSMin(((exports) => {
	exports.foo = 123;
}));

//#endregion
//#region entry.ts
var import_foo = /* @__PURE__ */ __toESM(require_foo());
console.log(import_foo.foo, import_foo.foo, 234);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,5 @@
-var require_foo = __commonJS({
-    "foo.ts"(exports) {
-        exports.foo = 123;
-    }
+var require_foo = __commonJSMin(exports => {
+    exports.foo = 123;
 });
-var ns = __toESM(require_foo());
-var foo2 = 234;
-console.log(ns.foo, ns.foo, foo2);
+var import_foo = __toESM(require_foo());
+console.log(import_foo.foo, import_foo.foo, 234);

```