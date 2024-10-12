# Diff
## /out.js
### esbuild
```js
var entry_exports = {};
__export(entry_exports, {
  bar: () => import_bar.default,
  foo: () => import_foo.default
});
module.exports = __toCommonJS(entry_exports);
var import_foo = __toESM(require("./foo"));
var import_bar = __toESM(require("./bar"));
```
### rolldown
```js
import { default as foo } from "./foo";
import { default as bar } from "./bar";

export { bar, foo };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,8 +1,3 @@
-var entry_exports = {};
-__export(entry_exports, {
-    bar: () => import_bar.default,
-    foo: () => import_foo.default
-});
-module.exports = __toCommonJS(entry_exports);
-var import_foo = __toESM(require("./foo"));
-var import_bar = __toESM(require("./bar"));
+import {default as foo} from "./foo";
+import {default as bar} from "./bar";
+export {bar, foo};

```