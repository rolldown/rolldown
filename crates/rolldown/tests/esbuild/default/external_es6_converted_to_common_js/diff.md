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
import * as ns$3 from "x";
import * as ns$2 from "x";
import * as ns$1 from "x";
import { ns } from "x";


//#region a.js
var a_exports;
var init_a = __esm({ "a.js"() {
	a_exports = {};
	__export(a_exports, { ns: () => ns$3 });
} });

//#endregion
//#region b.js
var b_exports;
var init_b = __esm({ "b.js"() {
	b_exports = {};
	__export(b_exports, { ns: () => ns$2 });
} });

//#endregion
//#region c.js
var c_exports;
var init_c = __esm({ "c.js"() {
	c_exports = {};
	__export(c_exports, { ns: () => ns$1 });
} });

//#endregion
//#region d.js
var d_exports;
var init_d = __esm({ "d.js"() {
	d_exports = {};
	__export(d_exports, { ns: () => ns });
} });

//#endregion
//#region e.js
import * as import_x from "x";
var e_exports;
var init_e = __esm({ "e.js"() {
	e_exports = {};
	__reExport(e_exports, import_x);
} });

//#endregion
//#region entry.js
init_a(), __toCommonJS(a_exports);
init_b(), __toCommonJS(b_exports);
init_c(), __toCommonJS(c_exports);
init_d(), __toCommonJS(d_exports);
init_e(), __toCommonJS(e_exports);

//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,44 +1,53 @@
-var a_exports = {};
-__export(a_exports, {
-    ns: () => ns
-});
-import * as ns from "x";
+import * as ns$3 from "x";
+import * as ns$2 from "x";
+import * as ns$1 from "x";
+import {ns} from "x";
+var a_exports;
 var init_a = __esm({
-    "a.js"() {}
+    "a.js"() {
+        a_exports = {};
+        __export(a_exports, {
+            ns: () => ns$3
+        });
+    }
 });
-var b_exports = {};
-__export(b_exports, {
-    ns: () => ns2
-});
-import * as ns2 from "x";
+var b_exports;
 var init_b = __esm({
-    "b.js"() {}
+    "b.js"() {
+        b_exports = {};
+        __export(b_exports, {
+            ns: () => ns$2
+        });
+    }
 });
-var c_exports = {};
-__export(c_exports, {
-    ns: () => ns3
-});
-import * as ns3 from "x";
+var c_exports;
 var init_c = __esm({
-    "c.js"() {}
+    "c.js"() {
+        c_exports = {};
+        __export(c_exports, {
+            ns: () => ns$1
+        });
+    }
 });
-var d_exports = {};
-__export(d_exports, {
-    ns: () => ns4
-});
-import {ns as ns4} from "x";
+var d_exports;
 var init_d = __esm({
-    "d.js"() {}
+    "d.js"() {
+        d_exports = {};
+        __export(d_exports, {
+            ns: () => ns
+        });
+    }
 });
-var e_exports = {};
-import * as x_star from "x";
+import * as import_x from "x";
+var e_exports;
 var init_e = __esm({
     "e.js"() {
-        __reExport(e_exports, x_star);
+        e_exports = {};
+        __reExport(e_exports, import_x);
     }
 });
-init_a();
-init_b();
-init_c();
-init_d();
-init_e();
+(init_a(), __toCommonJS(a_exports));
+(init_b(), __toCommonJS(b_exports));
+(init_c(), __toCommonJS(c_exports));
+(init_d(), __toCommonJS(d_exports));
+(init_e(), __toCommonJS(e_exports));

```