# Reason
1. esbuild enable minify syntax this could be done in minifier, rest part should be same
# Diff
## /out.js
### esbuild
```js
(() => {
  let e = 1;
  let outer = 2;
  let outerDead = 3;
  with ({}) {
    var hoisted = 4;
    let t = 5;
    hoisted++;
    t++;
    if (1) outer++;
    if (0) outerDead++;
  }
  if (1) {
    hoisted++;
    e++;
    outer++;
    outerDead++;
  }
})();
```
### rolldown
```js

//#region entry.js
(() => {
	let local = 1;
	let outer = 2;
	let outerDead = 3;
	with({}) {
		var hoisted = 4;
		let local = 5;
		hoisted++;
		local++;
		if (1) outer++;
		if (0) outerDead++;
	}
	if (1) {
		hoisted++;
		local++;
		outer++;
		outerDead++;
	}
})();

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,19 +1,23 @@
+
+//#region entry.js
 (() => {
-  let e = 1;
-  let outer = 2;
-  let outerDead = 3;
-  with ({}) {
-    var hoisted = 4;
-    let t = 5;
-    hoisted++;
-    t++;
-    if (1) outer++;
-    if (0) outerDead++;
-  }
-  if (1) {
-    hoisted++;
-    e++;
-    outer++;
-    outerDead++;
-  }
-})();
\ No newline at end of file
+	let local = 1;
+	let outer = 2;
+	let outerDead = 3;
+	with({}) {
+		var hoisted = 4;
+		let local = 5;
+		hoisted++;
+		local++;
+		if (1) outer++;
+		if (0) outerDead++;
+	}
+	if (1) {
+		hoisted++;
+		local++;
+		outer++;
+		outerDead++;
+	}
+})();
+
+//#endregion
\ No newline at end of file

```