# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/src/foo-require.js
var require_foo_require = __commonJS({
  "Users/user/project/src/foo-require.js"(exports, module) {
    module.exports = "foo";
  }
});

// Users/user/project/src/index.js
var require_src = __commonJS({
  "Users/user/project/src/index.js"(exports, module) {
    module.exports = "index";
    console.log(
      require_src(),
      require_foo_require()
    );
  }
});
export default require_src();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	
@@ -1,12 +0,0 @@
-var require_foo_require = __commonJS({
-    "Users/user/project/src/foo-require.js"(exports, module) {
-        module.exports = "foo";
-    }
-});
-var require_src = __commonJS({
-    "Users/user/project/src/index.js"(exports, module) {
-        module.exports = "index";
-        console.log(require_src(), require_foo_require());
-    }
-});
-export default require_src();

```