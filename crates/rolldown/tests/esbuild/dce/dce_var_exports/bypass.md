# Reason
1. We try to extract common runtime into another chunks, everything else are just same
# Diff
## /out/a.js
### esbuild
```js
// a.js
var require_a = __commonJS({
  "a.js"(exports, module) {
    var foo = { bar: 123 };
    module.exports = foo;
  }
});
export default require_a();
```
### rolldown
```js
import { __commonJS } from "./chunk.js";

//#region a.js
var require_a = __commonJS({ "a.js"(exports, module) {
	var foo = { bar: 123 };
	module.exports = foo;
} });

export default require_a();

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	a.js
@@ -1,4 +1,5 @@
+import {__commonJS} from "./chunk.js";
 var require_a = __commonJS({
     "a.js"(exports, module) {
         var foo = {
             bar: 123

```
## /out/b.js
### esbuild
```js
// b.js
var require_b = __commonJS({
  "b.js"(exports, module) {
    var exports = { bar: 123 };
    module.exports = exports;
  }
});
export default require_b();
```
### rolldown
```js
import { __commonJS } from "./chunk.js";

//#region b.js
var require_b = __commonJS({ "b.js"(exports, module) {
	var exports = { bar: 123 };
	module.exports = exports;
} });

export default require_b();

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	b.js
@@ -1,4 +1,5 @@
+import {__commonJS} from "./chunk.js";
 var require_b = __commonJS({
     "b.js"(exports, module) {
         var exports = {
             bar: 123

```
## /out/c.js
### esbuild
```js
// c.js
var require_c = __commonJS({
  "c.js"(exports, module) {
    var module = { bar: 123 };
    exports.foo = module;
  }
});
export default require_c();
```
### rolldown
```js
import { __commonJS } from "./chunk.js";

//#region c.js
var require_c = __commonJS({ "c.js"(exports) {
	var module = { bar: 123 };
	exports.foo = module;
} });

export default require_c();

```
### diff
```diff
===================================================================
--- esbuild	/out/c.js
+++ rolldown	c.js
@@ -1,6 +1,7 @@
+import {__commonJS} from "./chunk.js";
 var require_c = __commonJS({
-    "c.js"(exports, module) {
+    "c.js"(exports) {
         var module = {
             bar: 123
         };
         exports.foo = module;

```