# Diff
## /out/test.js
### esbuild
```js
// project/node_modules/pkg/button.jsx
var Button = () => /* @__PURE__ */ React.createElement("button", null);

// project/test.jsx
render(/* @__PURE__ */ React.createElement(Button, null));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/test.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var Button = () => React.createElement("button", null);
-render(React.createElement(Button, null));

```