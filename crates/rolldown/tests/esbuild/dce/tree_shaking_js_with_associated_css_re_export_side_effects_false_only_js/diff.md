# Diff
## /out/test.js
### esbuild
```js
// project/node_modules/pkg/button.css
var require_button = __commonJS({
  "project/node_modules/pkg/button.css"(exports, module) {
    module.exports = {};
  }
});

// project/node_modules/pkg/components.jsx
require_button();
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
@@ -1,8 +0,0 @@
-var require_button = __commonJS({
-    "project/node_modules/pkg/button.css"(exports, module) {
-        module.exports = {};
-    }
-});
-require_button();
-var Button = () => React.createElement("button", null);
-render(React.createElement(Button, null));

```