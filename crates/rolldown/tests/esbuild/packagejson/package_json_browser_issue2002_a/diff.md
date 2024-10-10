# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/node_modules/sub/bar.js
var require_bar = __commonJS({
  "Users/user/project/src/node_modules/sub/bar.js"() {
    works();
  }
});

// Users/user/project/src/node_modules/pkg/sub/foo.js
var require_foo = __commonJS({
  "Users/user/project/src/node_modules/pkg/sub/foo.js"() {
    require_bar();
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
-var require_bar = __commonJS({
-    "Users/user/project/src/node_modules/sub/bar.js"() {
-        works();
-    }
-});
-var require_foo = __commonJS({
-    "Users/user/project/src/node_modules/pkg/sub/foo.js"() {
-        require_bar();
-    }
-});
-require_foo();

```