# Reason
1. different chunk naming style
# Diff
## /out/entry.js
### esbuild
```js
// entry.js
import("./foo-X6C7FV5C.js").then(({ default: { bar } }) => console.log(bar));
```
### rolldown
```js
import { __toDynamicImportESM } from "./chunk.js";

//#region entry.js
import("./foo.js").then(__toDynamicImportESM()).then(({ default: { bar } }) => console.log(bar));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,1 +1,2 @@
-import("./foo-X6C7FV5C.js").then(({default: {bar}}) => console.log(bar));
+import {__toDynamicImportESM} from "./chunk.js";
+import("./foo.js").then(__toDynamicImportESM()).then(({default: {bar}}) => console.log(bar));

```
## /out/foo-X6C7FV5C.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports) {
    exports.bar = 123;
  }
});
export default require_foo();
```
### rolldown
```js
import { __commonJS } from "./chunk.js";

//#region foo.js
var require_foo = __commonJS({ "foo.js"(exports) {
	exports.bar = 123;
} });

//#endregion
export default require_foo();

```
### diff
```diff
===================================================================
--- esbuild	/out/foo-X6C7FV5C.js
+++ rolldown	foo.js
@@ -1,4 +1,5 @@
+import {__commonJS} from "./chunk.js";
 var require_foo = __commonJS({
     "foo.js"(exports) {
         exports.bar = 123;
     }

```