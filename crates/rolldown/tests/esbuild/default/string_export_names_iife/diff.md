# Diff
## /out.js
### esbuild
```js
var global;
(global ||= {}).name = (() => {
  var entry_exports = {};
  __export(entry_exports, {
    "all the stuff": () => all_the_stuff,
    "some export": () => import_foo["some import"]
  });
  var import_foo = require("./foo");
  var all_the_stuff = __toESM(require("./foo"));
  return __toCommonJS(entry_exports);
})();
```
### rolldown
```js
import * as all the stuff from "./foo";
import { some import as someImport } from "./foo";

export { all the stuff, someImport as 'some export' };

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,4 @@
-var global;
-(global ||= {}).name = (() => {
-    var entry_exports = {};
-    __export(entry_exports, {
-        "all the stuff": () => all_the_stuff,
-        "some export": () => import_foo["some import"]
-    });
-    var import_foo = require("./foo");
-    var all_the_stuff = __toESM(require("./foo"));
-    return __toCommonJS(entry_exports);
-})();
+import * as all the stuff from "./foo";
+import { some import as someImport } from "./foo";
+
+export { all the stuff, someImport as 'some export' };

```