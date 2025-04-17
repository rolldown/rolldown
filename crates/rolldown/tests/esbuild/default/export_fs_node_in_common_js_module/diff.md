# Diff
## /out.js
### esbuild
```js
// entry.js
import * as fs from "fs";
import { readFileSync } from "fs";
var require_entry = __commonJS({
  "entry.js"(exports) {
    exports.fs = fs;
    exports.readFileSync = readFileSync;
    exports.foo = 123;
  }
});
export default require_entry();
```
### rolldown
```js
import * as fs from "fs";
import { readFileSync } from "fs";

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports) {
	exports.fs = fs;
	exports.readFileSync = readFileSync;
	exports.foo = 123;
} });

export default require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,12 @@
 import * as fs from "fs";
 import {readFileSync} from "fs";
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
 var require_entry = __commonJS({
     "entry.js"(exports) {
         exports.fs = fs;
         exports.readFileSync = readFileSync;

```