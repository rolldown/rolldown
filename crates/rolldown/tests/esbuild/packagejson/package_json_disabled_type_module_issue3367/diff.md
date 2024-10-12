# Diff
## /out.js
### esbuild
```js
// (disabled):node_modules/foo/index.js
var require_foo = __commonJS({
  "(disabled):node_modules/foo/index.js"() {
  }
});

// entry.js
var import_foo = __toESM(require_foo());
(0, import_foo.default)();
```
### rolldown
```js


//#region (ignored) 
var require_package_json_disabled_type_module_issue3367 = __commonJS({ ""() {} });

//#endregion
//#region entry.js
var import_package_json_disabled_type_module_issue3367 = __toESM(require_package_json_disabled_type_module_issue3367());
(0, import_package_json_disabled_type_module_issue3367.default)();

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,5 @@
-var require_foo = __commonJS({
-    "(disabled):node_modules/foo/index.js"() {}
+var require_package_json_disabled_type_module_issue3367 = __commonJS({
+    ""() {}
 });
-var import_foo = __toESM(require_foo());
-(0, import_foo.default)();
+var import_package_json_disabled_type_module_issue3367 = __toESM(require_package_json_disabled_type_module_issue3367());
+(0, import_package_json_disabled_type_module_issue3367.default)();

```