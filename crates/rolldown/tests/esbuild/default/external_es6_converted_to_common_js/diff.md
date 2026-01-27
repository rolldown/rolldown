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

// HIDDEN [\0rolldown/runtime.js]
//#region a.js
var a_exports = /* @__PURE__ */ __exportAll({ ns: () => ns$1 });
var init_a = __esmMin((() => {}));

//#endregion
//#region b.js
var b_exports = /* @__PURE__ */ __exportAll({ ns: () => ns$1 });
var init_b = __esmMin((() => {}));

//#endregion
//#region c.js
var c_exports = /* @__PURE__ */ __exportAll({ ns: () => ns$1 });
var init_c = __esmMin((() => {}));

//#endregion
//#region d.js
var d_exports = /* @__PURE__ */ __exportAll({ ns: () => ns });
var init_d = __esmMin((() => {}));

//#endregion
//#region e.js
var e_exports = /* @__PURE__ */ __exportAll({});
import * as import_x from "x";
__reExport(e_exports, import_x);
var init_e = __esmMin((() => {}));

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
@@ -1,43 +1,26 @@
-var a_exports = {};
-__export(a_exports, {
-    ns: () => ns
+import * as ns$1 from "x";
+import {ns} from "x";
+var a_exports = __exportAll({
+    ns: () => ns$1
 });
-import * as ns from "x";
-var init_a = __esm({
-    "a.js"() {}
+var init_a = __esmMin(() => {});
+var b_exports = __exportAll({
+    ns: () => ns$1
 });
-var b_exports = {};
-__export(b_exports, {
-    ns: () => ns2
+var init_b = __esmMin(() => {});
+var c_exports = __exportAll({
+    ns: () => ns$1
 });
-import * as ns2 from "x";
-var init_b = __esm({
-    "b.js"() {}
+var init_c = __esmMin(() => {});
+var d_exports = __exportAll({
+    ns: () => ns
 });
-var c_exports = {};
-__export(c_exports, {
-    ns: () => ns3
-});
-import * as ns3 from "x";
-var init_c = __esm({
-    "c.js"() {}
-});
-var d_exports = {};
-__export(d_exports, {
-    ns: () => ns4
-});
-import {ns as ns4} from "x";
-var init_d = __esm({
-    "d.js"() {}
-});
-var e_exports = {};
-import * as x_star from "x";
-var init_e = __esm({
-    "e.js"() {
-        __reExport(e_exports, x_star);
-    }
-});
+var init_d = __esmMin(() => {});
+var e_exports = __exportAll({});
+import * as import_x from "x";
+__reExport(e_exports, import_x);
+var init_e = __esmMin(() => {});
 init_a();
 init_b();
 init_c();
 init_d();

```