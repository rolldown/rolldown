# Diff
## /out.js
### esbuild
```js
(() => {
  // c.js
  var c_exports = {};
  var init_c = __esm({
    "c.js"() {
      if (false) for (let x of y) ;
    }
  });

  // b.js
  var b_exports = {};
  var init_b = __esm({
    "b.js"() {
      init_c();
    }
  });

  // a.js
  var a_exports = {};
  var init_a = __esm({
    "a.js"() {
      init_b();
    }
  });

  // entry.js
  var entry_exports = {};
  var init_entry = __esm({
    "entry.js"() {
      init_a();
      init_b();
      init_c();
      init_entry();
      if (false) for (let x of y) ;
    }
  });
  init_entry();
})();
```
### rolldown
```js


//#region c.js
var require_c = __commonJS({ "c.js"() {} });

//#endregion
//#region b.js
var b_exports;
var init_b = __esm({ "b.js"() {
	b_exports = {};
} });

//#endregion
//#region a.js
var a_exports;
var init_a = __esm({ "a.js"() {
	a_exports = {};
	init_b();
} });

//#endregion
//#region entry.js
var require_entry = __commonJS({ "entry.js"() {
	init_a(), __toCommonJS(a_exports);
	init_b(), __toCommonJS(b_exports);
	require_c();
	require_entry();
} });

//#endregion
export default require_entry();


```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,31 +1,25 @@
-(() => {
-    var c_exports = {};
-    var init_c = __esm({
-        "c.js"() {
-            if (false) for (let x of y) ;
-        }
-    });
-    var b_exports = {};
-    var init_b = __esm({
-        "b.js"() {
-            init_c();
-        }
-    });
-    var a_exports = {};
-    var init_a = __esm({
-        "a.js"() {
-            init_b();
-        }
-    });
-    var entry_exports = {};
-    var init_entry = __esm({
-        "entry.js"() {
-            init_a();
-            init_b();
-            init_c();
-            init_entry();
-            if (false) for (let x of y) ;
-        }
-    });
-    init_entry();
-})();
+var require_c = __commonJS({
+    "c.js"() {}
+});
+var b_exports;
+var init_b = __esm({
+    "b.js"() {
+        b_exports = {};
+    }
+});
+var a_exports;
+var init_a = __esm({
+    "a.js"() {
+        a_exports = {};
+        init_b();
+    }
+});
+var require_entry = __commonJS({
+    "entry.js"() {
+        (init_a(), __toCommonJS(a_exports));
+        (init_b(), __toCommonJS(b_exports));
+        require_c();
+        require_entry();
+    }
+});
+export default require_entry();

```