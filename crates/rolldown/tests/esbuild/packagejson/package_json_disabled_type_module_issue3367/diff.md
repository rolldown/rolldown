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
//#endregion
//#region entry.js
(void 0)();
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,1 @@
-var require_foo = __commonJS({
-    "(disabled):node_modules/foo/index.js"() {}
-});
-var import_foo = __toESM(require_foo());
-(0, import_foo.default)();
+(void 0)();

```