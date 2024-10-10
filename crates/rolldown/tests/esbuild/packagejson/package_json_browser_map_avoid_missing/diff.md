# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/component-indexof/index.js
var require_component_indexof = __commonJS({
  "Users/user/project/node_modules/component-indexof/index.js"(exports, module) {
    module.exports = function() {
      return 234;
    };
  }
});

// Users/user/project/node_modules/component-classes/index.js
try {
  index = require_component_indexof();
} catch (err) {
  index = require_component_indexof();
}
var index;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	
@@ -1,13 +0,0 @@
-var require_component_indexof = __commonJS({
-    "Users/user/project/node_modules/component-indexof/index.js"(exports, module) {
-        module.exports = function () {
-            return 234;
-        };
-    }
-});
-try {
-    index = require_component_indexof();
-} catch (err) {
-    index = require_component_indexof();
-}
-var index;

```