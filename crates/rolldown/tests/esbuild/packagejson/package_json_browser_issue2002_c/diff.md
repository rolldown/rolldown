# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/node_modules/sub/index.js
var require_sub = __commonJS({
  "Users/user/project/src/node_modules/sub/index.js"() {
    works();
  }
});

// Users/user/project/src/node_modules/pkg/sub/foo.js
var require_foo = __commonJS({
  "Users/user/project/src/node_modules/pkg/sub/foo.js"() {
    require_sub();
  }
});

// Users/user/project/src/entry.js
require_foo();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	
@@ -1,11 +0,0 @@
-var require_sub = __commonJS({
-    "Users/user/project/src/node_modules/sub/index.js"() {
-        works();
-    }
-});
-var require_foo = __commonJS({
-    "Users/user/project/src/node_modules/pkg/sub/foo.js"() {
-        require_sub();
-    }
-});
-require_foo();

```