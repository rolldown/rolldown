# Diff
## /out/entry.js
### esbuild
```js
// node_modules/fs/abc.js
var require_abc = __commonJS({
  "node_modules/fs/abc.js"() {
    console.log("include this");
  }
});

// node_modules/fs/index.js
var require_fs = __commonJS({
  "node_modules/fs/index.js"() {
    console.log("include this too");
  }
});

// entry.js
console.log([
  // These are node core modules
  require("fs"),
  require("fs/promises"),
  require("node:foo"),
  // These are not node core modules
  require_abc(),
  require_fs()
]);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,11 +0,0 @@
-var require_abc = __commonJS({
-    "node_modules/fs/abc.js"() {
-        console.log("include this");
-    }
-});
-var require_fs = __commonJS({
-    "node_modules/fs/index.js"() {
-        console.log("include this too");
-    }
-});
-console.log([require("fs"), require("fs/promises"), require("node:foo"), require_abc(), require_fs()]);

```