# Reason 
1. cjs module lexer can't recognize esbuild interop pattern
# Diff
## /out.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports) {
    exports.foo = 123;
  }
});

// entry.js
var entry_exports = {};
__export(entry_exports, {
  bar: () => import_foo.bar
});
module.exports = __toCommonJS(entry_exports);
var import_foo = __toESM(require_foo());
```
### rolldown
```js
// HIDDEN [rolldown:runtime]

//#region foo.js
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.foo = 123;
} });

//#endregion
//#region entry.js
var import_foo = __toESM(require_foo());

//#endregion
Object.defineProperty(exports, 'bar', {
  enumerable: true,
  get: function () {
    return import_foo.bar;
  }
});
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -2,10 +2,11 @@
     "foo.js"(exports) {
         exports.foo = 123;
     }
 });
-var entry_exports = {};
-__export(entry_exports, {
-    bar: () => import_foo.bar
-});
-module.exports = __toCommonJS(entry_exports);
 var import_foo = __toESM(require_foo());
+Object.defineProperty(exports, 'bar', {
+    enumerable: true,
+    get: function () {
+        return import_foo.bar;
+    }
+});

```