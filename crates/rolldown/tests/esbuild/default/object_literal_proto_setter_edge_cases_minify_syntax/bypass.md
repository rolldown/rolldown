# Reason
1. different naming style
# Diff
## /out/import-computed.js
### esbuild
```js
import { __proto__, bar } from "foo";
function foo() {
  console.log(
    'this must not become "{ __proto__: ... }":',
    {
      ["__proto__"]: __proto__,
      bar
    }
  );
}
```
### rolldown
```js
import { __proto__, bar } from "foo";

//#region import-computed.js
function foo() {
	console.log("this must not become \"{ __proto__: ... }\":", {
		["__proto__"]: __proto__,
		["bar"]: bar
	});
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/import-computed.js
+++ rolldown	import-computed.js
@@ -1,7 +1,7 @@
 import {__proto__, bar} from "foo";
 function foo() {
-    console.log('this must not become "{ __proto__: ... }":', {
+    console.log("this must not become \"{ __proto__: ... }\":", {
         ["__proto__"]: __proto__,
-        bar
+        ["bar"]: bar
     });
 }

```
## /out/import-normal.js
### esbuild
```js
import { __proto__, bar } from "foo";
function foo() {
  console.log(
    'this must not become "{ __proto__ }":',
    {
      __proto__: __proto__,
      bar
    }
  );
}
```
### rolldown
```js
import { __proto__, bar } from "foo";

//#region import-normal.js
function foo() {
	console.log("this must not become \"{ __proto__ }\":", {
		__proto__: __proto__,
		bar
	});
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/import-normal.js
+++ rolldown	import-normal.js
@@ -1,7 +1,7 @@
 import {__proto__, bar} from "foo";
 function foo() {
-    console.log('this must not become "{ __proto__ }":', {
+    console.log("this must not become \"{ __proto__ }\":", {
         __proto__: __proto__,
         bar
     });
 }

```
## /out/local-computed.js
### esbuild
```js
function foo(__proto__, bar) {
  {
    let __proto__2, bar2;
    console.log(
      'this must not become "{ __proto__: ... }":',
      {
        ["__proto__"]: __proto__2,
        bar: bar2
      }
    );
  }
}
```
### rolldown
```js

//#region local-computed.js
function foo(__proto__, bar) {
	{
		let __proto__, bar;
		console.log("this must not become \"{ __proto__: ... }\":", {
			["__proto__"]: __proto__,
			["bar"]: bar
		});
	}
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/local-computed.js
+++ rolldown	local-computed.js
@@ -1,9 +1,9 @@
 function foo(__proto__, bar) {
     {
-        let __proto__2, bar2;
-        console.log('this must not become "{ __proto__: ... }":', {
-            ["__proto__"]: __proto__2,
-            bar: bar2
+        let __proto__, bar;
+        console.log("this must not become \"{ __proto__: ... }\":", {
+            ["__proto__"]: __proto__,
+            ["bar"]: bar
         });
     }
 }

```
## /out/local-normal.js
### esbuild
```js
function foo(__proto__, bar) {
  console.log(
    'this must not become "{ __proto__ }":',
    {
      __proto__: __proto__,
      bar
    }
  );
}
```
### rolldown
```js

//#region local-normal.js
function foo(__proto__, bar) {
	console.log("this must not become \"{ __proto__ }\":", {
		__proto__: __proto__,
		bar
	});
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/local-normal.js
+++ rolldown	local-normal.js
@@ -1,6 +1,6 @@
 function foo(__proto__, bar) {
-    console.log('this must not become "{ __proto__ }":', {
+    console.log("this must not become \"{ __proto__ }\":", {
         __proto__: __proto__,
         bar
     });
 }

```