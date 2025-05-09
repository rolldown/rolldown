# Reason
1. could be done in minifier
# Diff
## /out/entry-esm.js
### esbuild
```js
// cjs.js
var require_cjs = __commonJS({
  "cjs.js"(exports) {
    exports.a = "foo";
  }
});

// esm.js
var esm_exports = {};
__export(esm_exports, {
  esm_foo_: () => esm_foo_
});
var esm_foo_ = "foo";

// entry-esm.js
var import_cjs = __toESM(require_cjs());
var cjs = __toESM(require_cjs());
var bar_ = [
  esm_foo_,
  import_cjs.cjs_foo_,
  esm_exports.b,
  cjs.a
];
export {
  bar_
};
```
### rolldown
```js
import { __toESM, esm_foo_, init_esm, require_cjs } from "./cjs.js";

//#region entry-esm.js
init_esm();
var import_cjs = __toESM(require_cjs());
var import_cjs$1 = __toESM(require_cjs());
let bar_ = [
	esm_foo_,
	import_cjs.cjs_foo_,
	esm_foo_,
	import_cjs$1.cjs_foo_
];

//#endregion
export { bar_ };
```
### diff
```diff
===================================================================
--- esbuild	/out/entry-esm.js
+++ rolldown	entry-esm.js
@@ -1,14 +1,6 @@
-var require_cjs = __commonJS({
-    "cjs.js"(exports) {
-        exports.a = "foo";
-    }
-});
-var esm_exports = {};
-__export(esm_exports, {
-    esm_foo_: () => esm_foo_
-});
-var esm_foo_ = "foo";
+import {__toESM, esm_foo_, init_esm, require_cjs} from "./cjs.js";
+init_esm();
 var import_cjs = __toESM(require_cjs());
-var cjs = __toESM(require_cjs());
-var bar_ = [esm_foo_, import_cjs.cjs_foo_, esm_exports.b, cjs.a];
+var import_cjs$1 = __toESM(require_cjs());
+var bar_ = [esm_foo_, import_cjs.cjs_foo_, esm_foo_, import_cjs$1.cjs_foo_];
 export {bar_};

```
## /out/entry-cjs.js
### esbuild
```js
// esm.js
var esm_exports = {};
__export(esm_exports, {
  esm_foo_: () => esm_foo_
});
var esm_foo_;
var init_esm = __esm({
  "esm.js"() {
    esm_foo_ = "foo";
  }
});

// cjs.js
var require_cjs = __commonJS({
  "cjs.js"(exports) {
    exports.a = "foo";
  }
});

// entry-cjs.js
var require_entry_cjs = __commonJS({
  "entry-cjs.js"(exports) {
    var { b: esm_foo_2 } = (init_esm(), __toCommonJS(esm_exports));
    var { a: cjs_foo_ } = require_cjs();
    exports.c = [
      esm_foo_2,
      cjs_foo_
    ];
  }
});
export default require_entry_cjs();
```
### rolldown
```js
import { __commonJS, __toCommonJS, esm_exports, init_esm, require_cjs } from "./cjs.js";

//#region entry-cjs.js
var require_entry_cjs = __commonJS({ "entry-cjs.js"(exports) {
	let { esm_foo_ } = (init_esm(), __toCommonJS(esm_exports));
	let { cjs_foo_ } = require_cjs();
	exports.bar_ = [esm_foo_, cjs_foo_];
} });

//#endregion
export default require_entry_cjs();

```
### diff
```diff
===================================================================
--- esbuild	/out/entry-cjs.js
+++ rolldown	entry-cjs.js
@@ -1,23 +1,9 @@
-var esm_exports = {};
-__export(esm_exports, {
-    esm_foo_: () => esm_foo_
-});
-var esm_foo_;
-var init_esm = __esm({
-    "esm.js"() {
-        esm_foo_ = "foo";
-    }
-});
-var require_cjs = __commonJS({
-    "cjs.js"(exports) {
-        exports.a = "foo";
-    }
-});
+import {__commonJS, __toCommonJS, esm_exports, init_esm, require_cjs} from "./cjs.js";
 var require_entry_cjs = __commonJS({
     "entry-cjs.js"(exports) {
-        var {b: esm_foo_2} = (init_esm(), __toCommonJS(esm_exports));
-        var {a: cjs_foo_} = require_cjs();
-        exports.c = [esm_foo_2, cjs_foo_];
+        let {esm_foo_} = (init_esm(), __toCommonJS(esm_exports));
+        let {cjs_foo_} = require_cjs();
+        exports.bar_ = [esm_foo_, cjs_foo_];
     }
 });
 export default require_entry_cjs();

```