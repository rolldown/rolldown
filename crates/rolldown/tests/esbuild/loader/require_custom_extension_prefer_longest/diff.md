# Reason
1. `.txt` should be treated as cjs
# Diff
## /out.js
### esbuild
```js
// test.txt
var require_test = __commonJS({
  "test.txt"(exports, module) {
    module.exports = "test.txt";
  }
});

// test.base64.txt
var require_test_base64 = __commonJS({
  "test.base64.txt"(exports, module) {
    module.exports = "dGVzdC5iYXNlNjQudHh0";
  }
});

// entry.js
console.log(require_test(), require_test_base64());
```
### rolldown
```js


//#region test.txt
var require_test = __commonJS({ "test.txt"(exports, module) {
	module.exports = "test.txt";
} });

//#endregion
//#region test.base64.txt
var require_test_base64 = __commonJS({ "test.base64.txt"(exports, module) {
	module.exports = "dGVzdC5iYXNlNjQudHh0";
} });

//#endregion
//#region entry.js
<<<<<<< HEAD
console.log(require_test(), require_test_base64());
||||||| 1fa5d4b54
console.log((init_test(), __toCommonJS(test_exports)), (init_test_base64(), __toCommonJS(test_base64_exports)));
=======
console.log(require_test(), (init_test_base64(), __toCommonJS(test_base64_exports)));
>>>>>>> main

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,11 +1,24 @@
-var require_test = __commonJS({
-    "test.txt"(exports, module) {
-        module.exports = "test.txt";
-    }
-});
-var require_test_base64 = __commonJS({
-    "test.base64.txt"(exports, module) {
-        module.exports = "dGVzdC5iYXNlNjQudHh0";
-    }
-});
+
+
+//#region test.txt
+var require_test = __commonJS({ "test.txt"(exports, module) {
+	module.exports = "test.txt";
+} });
+
+//#endregion
+//#region test.base64.txt
+var require_test_base64 = __commonJS({ "test.base64.txt"(exports, module) {
+	module.exports = "dGVzdC5iYXNlNjQudHh0";
+} });
+
+//#endregion
+//#region entry.js
+<<<<<<< HEAD
 console.log(require_test(), require_test_base64());
+||||||| 1fa5d4b54
+console.log((init_test(), __toCommonJS(test_exports)), (init_test_base64(), __toCommonJS(test_base64_exports)));
+=======
+console.log(require_test(), (init_test_base64(), __toCommonJS(test_base64_exports)));
+>>>>>>> main
+
+//#endregion
\ No newline at end of file

```