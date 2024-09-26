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


//#region foo.js
var require_foo = __commonJSMin((exports) => {
	exports.foo = 123;
});

//#endregion
//#region entry.js
var import_foo = __toESM(require_foo());

//#endregion
var bar = import_foo.bar;
Object.defineProperty(exports, 'bar', {
  enumerable: true,
  get: function () {
    return bar;
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
-__export(entry_exports, { bar: () => import_foo.bar });
-module.exports = __toCommonJS(entry_exports);
-var import_foo = __toESM(require_foo());
\ No newline at end of file
+var import_foo = __toESM(require_foo());
+var bar = import_foo.bar;
+Object.defineProperty(exports, 'bar', {
+    enumerable: true,
+    get: function () {
+        return bar;
+    }
+});
\ No newline at end of file

```
