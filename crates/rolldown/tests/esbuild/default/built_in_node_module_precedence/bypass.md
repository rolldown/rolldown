# Reason
1. different naming style
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


//#region node_modules/fs/abc.js
var require_abc = __commonJS({ "node_modules/fs/abc.js"() {
	console.log("include this");
} });

//#endregion
//#region node_modules/fs/index.js
var require_fs_index = __commonJS({ "node_modules/fs/index.js"() {
	console.log("include this too");
} });

//#endregion
//#region entry.js
console.log([
	require("fs"),
	require("fs/promises"),
	require("node:foo"),
	require_abc(),
	require_fs_index()
]);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -2,10 +2,10 @@
     "node_modules/fs/abc.js"() {
         console.log("include this");
     }
 });
-var require_fs = __commonJS({
+var require_fs_index = __commonJS({
     "node_modules/fs/index.js"() {
         console.log("include this too");
     }
 });
-console.log([require("fs"), require("fs/promises"), require("node:foo"), require_abc(), require_fs()]);
+console.log([require("fs"), require("fs/promises"), require("node:foo"), require_abc(), require_fs_index()]);

```