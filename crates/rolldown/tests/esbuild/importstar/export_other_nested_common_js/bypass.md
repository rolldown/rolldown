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
  y: () => import_foo.x
});
module.exports = __toCommonJS(entry_exports);

// bar.js
var import_foo = __toESM(require_foo());
```
### rolldown
```js


//#region foo.js
var import_foo;
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.foo = 123;
	import_foo = __toESM(require_foo());
} });

//#endregion
//#region bar.js
require_foo();

//#endregion
Object.defineProperty(exports, 'y', {
  enumerable: true,
  get: function () {
    return import_foo.x;
  }
});
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,14 @@
+var import_foo;
 var require_foo = __commonJS({
     "foo.js"(exports) {
         exports.foo = 123;
+        import_foo = __toESM(require_foo());
     }
 });
-var entry_exports = {};
-__export(entry_exports, {
-    y: () => import_foo.x
+require_foo();
+Object.defineProperty(exports, 'y', {
+    enumerable: true,
+    get: function () {
+        return import_foo.x;
+    }
 });
-module.exports = __toCommonJS(entry_exports);
-var import_foo = __toESM(require_foo());

```