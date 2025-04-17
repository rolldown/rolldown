# Reason
1. css comments
2. different chunk file naming style
# Diff
## /out/main/js/entry1-4X3SO762.js
### esbuild
```js
import "../../common/js/chunk-XHGYOYUR.js";

// src/entries/entry1.js
console.log("entry1");
```
### rolldown
```js
import "./shared.js";

//#region entries/entry1.js
console.log("entry1");

```
### diff
```diff
===================================================================
--- esbuild	/out/main/js/entry1-4X3SO762.js
+++ rolldown	entries_entry1.js
@@ -1,2 +1,2 @@
-import "../../common/js/chunk-XHGYOYUR.js";
+import "./shared.js";
 console.log("entry1");

```
## /out/main/js/entry2-URQRHZS5.js
### esbuild
```js
import "../../common/js/chunk-XHGYOYUR.js";

// src/entries/entry2.js
console.log("entry2");
```
### rolldown
```js
import "./shared.js";

//#region entries/entry2.js
console.log("entry2");

```
### diff
```diff
===================================================================
--- esbuild	/out/main/js/entry2-URQRHZS5.js
+++ rolldown	entries_entry2.js
@@ -1,2 +1,2 @@
-import "../../common/js/chunk-XHGYOYUR.js";
+import "./shared.js";
 console.log("entry2");

```
## /out/main/css/entry1-3JZGIUSL.css
### esbuild
```js
/* src/entries/entry1.css */
a:after {
  content: "entry1";
}
```
### rolldown
```js
a:after { content: "entry1" }

```
### diff
```diff
===================================================================
--- esbuild	/out/main/css/entry1-3JZGIUSL.css
+++ rolldown	entries_entry1.css
@@ -1,4 +1,1 @@
-/* src/entries/entry1.css */
-a:after {
-  content: "entry1";
-}
\ No newline at end of file
+a:after { content: "entry1" }

```
## /out/main/css/entry2-NXZBPPIA.css
### esbuild
```js
/* src/entries/entry2.css */
a:after {
  content: "entry2";
}
```
### rolldown
```js
a:after { content: "entry2" }

```
### diff
```diff
===================================================================
--- esbuild	/out/main/css/entry2-NXZBPPIA.css
+++ rolldown	entries_entry2.css
@@ -1,4 +1,1 @@
-/* src/entries/entry2.css */
-a:after {
-  content: "entry2";
-}
\ No newline at end of file
+a:after { content: "entry2" }

```