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
  ns: () => ns
});
module.exports = __toCommonJS(entry_exports);
var ns = __toESM(require_foo());
```
### rolldown
```js


//#region foo.js
var require_foo = __commonJSMin((exports, module) => {
	exports.foo = 123;
});

//#endregion
//#region entry.js
var import_foo = __toESM(require_foo());

//#endregion
Object.defineProperty(exports, 'ns', {
  enumerable: true,
  get: function () {
    return import_foo;
  }
});

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry_js.cjs
@@ -1,9 +1,10 @@
-var require_foo = __commonJS({
-    'foo.js'(exports) {
-        exports.foo = 123;
-    }
+var require_foo = __commonJSMin((exports, module) => {
+    exports.foo = 123;
 });
-var entry_exports = {};
-__export(entry_exports, { ns: () => ns });
-module.exports = __toCommonJS(entry_exports);
-var ns = __toESM(require_foo());
\ No newline at end of file
+var import_foo = __toESM(require_foo());
+Object.defineProperty(exports, 'ns', {
+    enumerable: true,
+    get: function () {
+        return import_foo;
+    }
+});
\ No newline at end of file

```
