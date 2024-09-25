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
var require_foo = __commonJSMin((exports) => {
	exports.foo = 123;
});

//#endregion
//#region bar.js
var import_foo = __toESM(require_foo());

//#endregion
var y = import_foo.x;
Object.defineProperty(exports, 'y', {
  enumerable: true,
  get: function () {
    return y;
  }
});

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.cjs
@@ -1,9 +1,11 @@
-var require_foo = __commonJS({
-    'foo.js'(exports) {
-        exports.foo = 123;
-    }
+var require_foo = __commonJSMin(exports => {
+    exports.foo = 123;
 });
-var entry_exports = {};
-__export(entry_exports, { y: () => import_foo.x });
-module.exports = __toCommonJS(entry_exports);
-var import_foo = __toESM(require_foo());
\ No newline at end of file
+var import_foo = __toESM(require_foo());
+var y = import_foo.x;
+Object.defineProperty(exports, 'y', {
+    enumerable: true,
+    get: function () {
+        return y;
+    }
+});
\ No newline at end of file

```
