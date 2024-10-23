# Reason
1. needs css stable
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
import { jsx as _jsx, jsx as _jsx$1 } from "react/jsx-runtime";


//#region node_modules/pkg/button.css
var button_exports = {};
var init_button = __esm({ "node_modules/pkg/button.css"() {} });

//#endregion
//#region node_modules/pkg/components.jsx
init_button(), __toCommonJS(button_exports);
const Button = () => _jsx$1("button", {});

//#endregion
//#region test.jsx
render(_jsx(Button, {}));

//#endregion
//# sourceMappingURL=test.js.map
```
### diff
```diff
===================================================================
--- esbuild	/out/test.js
+++ rolldown	test.js
@@ -1,8 +1,8 @@
-var require_button = __commonJS({
-    "project/node_modules/pkg/button.css"(exports, module) {
-        module.exports = {};
-    }
+import {jsx as _jsx, jsx as _jsx$1} from "react/jsx-runtime";
+var button_exports = {};
+var init_button = __esm({
+    "node_modules/pkg/button.css"() {}
 });
-require_button();
-var Button = () => React.createElement("button", null);
-render(React.createElement(Button, null));
+(init_button(), __toCommonJS(button_exports));
+var Button = () => _jsx$1("button", {});
+render(_jsx(Button, {}));

```
## /out/test.css
### esbuild
```js
/* project/node_modules/pkg/button.css */
button {
  color: red;
}
```
### rolldown
```js
button { color: red }

```
### diff
```diff
===================================================================
--- esbuild	/out/test.css
+++ rolldown	test.css
@@ -1,4 +1,1 @@
-/* project/node_modules/pkg/button.css */
-button {
-  color: red;
-}
\ No newline at end of file
+button { color: red }

```