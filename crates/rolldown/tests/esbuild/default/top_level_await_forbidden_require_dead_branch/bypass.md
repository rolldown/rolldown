# Reason
1.  rolldown use `__commonJS` to wrap due to `oxc` DCE the dead branch, esbuild generate same output if remove the block
2. different iife impl
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
(function() {



//#region c.js
var require_c = __commonJS({ "c.js"() {} });

//#endregion
//#region b.js
var b_exports = {};
var init_b = __esm({ "b.js"() {} });

//#endregion
//#region a.js
var a_exports = {};
var init_a = __esm({ "a.js"() {
	init_b();
} });

//#endregion
//#region entry.js
var require_entry = __commonJS({ "entry.js"() {
	init_a();
	init_b();
	require_c();
	require_entry();
} });

//#endregion
return require_entry();

})();
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,31 +1,24 @@
-(() => {
-    var c_exports = {};
-    var init_c = __esm({
-        "c.js"() {
-            if (false) for (let x of y) ;
-        }
+(function () {
+    var require_c = __commonJS({
+        "c.js"() {}
     });
     var b_exports = {};
     var init_b = __esm({
-        "b.js"() {
-            init_c();
-        }
+        "b.js"() {}
     });
     var a_exports = {};
     var init_a = __esm({
         "a.js"() {
             init_b();
         }
     });
-    var entry_exports = {};
-    var init_entry = __esm({
+    var require_entry = __commonJS({
         "entry.js"() {
             init_a();
             init_b();
-            init_c();
-            init_entry();
-            if (false) for (let x of y) ;
+            require_c();
+            require_entry();
         }
     });
-    init_entry();
+    return require_entry();
 })();

```