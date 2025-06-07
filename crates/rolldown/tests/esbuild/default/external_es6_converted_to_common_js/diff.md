# Reason
1. redundant `import` statements
2. should not generate `__toCommonJS`
# Diff
## /out.js
### esbuild
```js
// a.js
var a_exports = {};
__export(a_exports, {
  ns: () => ns
});
import * as ns from "x";
var init_a = __esm({
  "a.js"() {
  }
});

// b.js
var b_exports = {};
__export(b_exports, {
  ns: () => ns2
});
import * as ns2 from "x";
var init_b = __esm({
  "b.js"() {
  }
});

// c.js
var c_exports = {};
__export(c_exports, {
  ns: () => ns3
});
import * as ns3 from "x";
var init_c = __esm({
  "c.js"() {
  }
});

// d.js
var d_exports = {};
__export(d_exports, {
  ns: () => ns4
});
import { ns as ns4 } from "x";
var init_d = __esm({
  "d.js"() {
  }
});

// e.js
var e_exports = {};
import * as x_star from "x";
var init_e = __esm({
  "e.js"() {
    __reExport(e_exports, x_star);
  }
});

// entry.js
init_a();
init_b();
init_c();
init_d();
init_e();
```
### rolldown
```js
import * as ns$1 from "x";
import { ns } from "x";


//#region a.js
var a_exports = {};
__export(a_exports, { ns: () => ns$1 });
var init_a = __esm({ "a.js"() {} });

//#endregion
//#region b.js
var b_exports = {};
__export(b_exports, { ns: () => ns$1 });
var init_b = __esm({ "b.js"() {} });

//#endregion
//#region c.js
var c_exports = {};
__export(c_exports, { ns: () => ns$1 });
var init_c = __esm({ "c.js"() {} });

//#endregion
//#region d.js
var d_exports = {};
__export(d_exports, { ns: () => ns });
var init_d = __esm({ "d.js"() {} });

//#endregion
//#region e.js
var e_exports = {};
import * as import_x from "x";
__reExport(e_exports, import_x);
var init_e = __esm({ "e.js"() {} });

//#endregion
//#region entry.js
init_a();
init_b();
init_c();
init_d();
init_e();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,42 +1,39 @@
+import * as ns$1 from "x";
+import {ns} from "x";
 var a_exports = {};
 __export(a_exports, {
-    ns: () => ns
+    ns: () => ns$1
 });
-import * as ns from "x";
 var init_a = __esm({
     "a.js"() {}
 });
 var b_exports = {};
 __export(b_exports, {
-    ns: () => ns2
+    ns: () => ns$1
 });
-import * as ns2 from "x";
 var init_b = __esm({
     "b.js"() {}
 });
 var c_exports = {};
 __export(c_exports, {
-    ns: () => ns3
+    ns: () => ns$1
 });
-import * as ns3 from "x";
 var init_c = __esm({
     "c.js"() {}
 });
 var d_exports = {};
 __export(d_exports, {
-    ns: () => ns4
+    ns: () => ns
 });
-import {ns as ns4} from "x";
 var init_d = __esm({
     "d.js"() {}
 });
 var e_exports = {};
-import * as x_star from "x";
+import * as import_x from "x";
+__reExport(e_exports, import_x);
 var init_e = __esm({
-    "e.js"() {
-        __reExport(e_exports, x_star);
-    }
+    "e.js"() {}
 });
 init_a();
 init_b();
 init_c();

```