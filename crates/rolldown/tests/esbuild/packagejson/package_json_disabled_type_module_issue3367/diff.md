# Reason
1. ignored module debug name seems not correct
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
var default$1 = void 0;

//#endregion
//#region entry.js
default$1();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,2 @@
-var require_foo = __commonJS({
-    "(disabled):node_modules/foo/index.js"() {}
-});
-var import_foo = __toESM(require_foo());
-(0, import_foo.default)();
+var default$1 = void 0;
+default$1();

```