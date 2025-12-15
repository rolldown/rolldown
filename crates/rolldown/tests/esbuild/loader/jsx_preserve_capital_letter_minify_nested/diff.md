## /out.js
### esbuild
```js
// entry.jsx
x = () => {
  class Y {
  }
  return <Y tag-must-start-with-capital-letter />;
};
```
### rolldown
```js
x=()=>{class e{}return<e tag-must-start-with-capital-letter/>};
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,1 @@
-// entry.jsx
-x = () => {
-  class Y {
-  }
-  return <Y tag-must-start-with-capital-letter />;
-};
\ No newline at end of file
+x=()=>{class e{}return<e tag-must-start-with-capital-letter/>};
\ No newline at end of file

```