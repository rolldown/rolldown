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

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	
@@ -1,5 +0,0 @@
-var require_foo = __commonJS({
-    "(disabled):node_modules/foo/index.js"() {}
-});
-var import_foo = __toESM(require_foo());
-(0, import_foo.default)();

```