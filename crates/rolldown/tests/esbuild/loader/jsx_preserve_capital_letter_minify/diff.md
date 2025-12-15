## /out.js
### esbuild
```js
// foo.js
var Y = class {
};

// entry.jsx
console.log(<Y tag-must-start-with-capital-letter />);
```
### rolldown
```js
var e=class{};console.log(<e tag-must-start-with-capital-letter/>);
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,1 @@
-// foo.js
-var Y = class {
-};
-
-// entry.jsx
-console.log(<Y tag-must-start-with-capital-letter />);
\ No newline at end of file
+var e=class{};console.log(<e tag-must-start-with-capital-letter/>);
\ No newline at end of file

```