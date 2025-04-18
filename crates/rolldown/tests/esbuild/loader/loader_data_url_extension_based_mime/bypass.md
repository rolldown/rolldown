# Reason
1. different deconflict naming style
# Diff
## /out/entry.js
### esbuild
```js
// example.css
var example_default = "data:text/css;charset=utf-8,css";

// example.eot
var example_default2 = "data:application/vnd.ms-fontobject,eot";

// example.gif
var example_default3 = "data:image/gif,gif";

// example.htm
var example_default4 = "data:text/html;charset=utf-8,htm";

// example.html
var example_default5 = "data:text/html;charset=utf-8,html";

// example.jpeg
var example_default6 = "data:image/jpeg,jpeg";

// example.jpg
var example_default7 = "data:image/jpeg,jpg";

// example.js
var example_default8 = "data:text/javascript;charset=utf-8,js";

// example.json
var example_default9 = "data:application/json;charset=utf-8,json";

// example.mjs
var example_default10 = "data:text/javascript;charset=utf-8,mjs";

// example.otf
var example_default11 = "data:font/otf,otf";

// example.pdf
var example_default12 = "data:application/pdf,pdf";

// example.png
var example_default13 = "data:image/png,png";

// example.sfnt
var example_default14 = "data:font/sfnt,sfnt";

// example.svg
var example_default15 = "data:image/svg+xml,svg";

// example.ttf
var example_default16 = "data:font/ttf,ttf";

// example.wasm
var example_default17 = "data:application/wasm,wasm";

// example.webp
var example_default18 = "data:image/webp,webp";

// example.woff
var example_default19 = "data:font/woff,woff";

// example.woff2
var example_default20 = "data:font/woff2,woff2";

// example.xml
var example_default21 = "data:text/xml;charset=utf-8,xml";
export {
  example_default as css,
  example_default2 as eot,
  example_default3 as gif,
  example_default4 as htm,
  example_default5 as html,
  example_default6 as jpeg,
  example_default7 as jpg,
  example_default8 as js,
  example_default9 as json,
  example_default10 as mjs,
  example_default11 as otf,
  example_default12 as pdf,
  example_default13 as png,
  example_default14 as sfnt,
  example_default15 as svg,
  example_default16 as ttf,
  example_default17 as wasm,
  example_default18 as webp,
  example_default19 as woff,
  example_default20 as woff2,
  example_default21 as xml
};
```
### rolldown
```js
//#region example.css
var example_default = "data:text/css;charset=utf-8,css";

//#endregion
//#region example.eot
var example_default$1 = "data:application/vnd.ms-fontobject,eot";

//#endregion
//#region example.gif
var example_default$2 = "data:image/gif,gif";

//#endregion
//#region example.htm
var example_default$3 = "data:text/html;charset=utf-8,htm";

//#endregion
//#region example.html
var example_default$4 = "data:text/html;charset=utf-8,html";

//#endregion
//#region example.jpeg
var example_default$5 = "data:image/jpeg,jpeg";

//#endregion
//#region example.jpg
var example_default$6 = "data:image/jpeg,jpg";

//#endregion
//#region example.js
var example_default$7 = "data:text/javascript;charset=utf-8,js";

//#endregion
//#region example.json
var example_default$8 = "data:application/json;charset=utf-8,json";

//#endregion
//#region example.mjs
var example_default$9 = "data:text/javascript;charset=utf-8,mjs";

//#endregion
//#region example.otf
var example_default$10 = "data:font/otf,otf";

//#endregion
//#region example.pdf
var example_default$11 = "data:application/pdf,pdf";

//#endregion
//#region example.png
var example_default$12 = "data:image/png,png";

//#endregion
//#region example.sfnt
var example_default$13 = "data:font/sfnt,sfnt";

//#endregion
//#region example.svg
var example_default$14 = "data:image/svg+xml,svg";

//#endregion
//#region example.ttf
var example_default$15 = "data:font/ttf,ttf";

//#endregion
//#region example.wasm
var example_default$16 = "data:application/wasm,wasm";

//#endregion
//#region example.webp
var example_default$17 = "data:image/webp,webp";

//#endregion
//#region example.woff
var example_default$18 = "data:font/woff,woff";

//#endregion
//#region example.woff2
var example_default$19 = "data:font/woff2,woff2";

//#endregion
//#region example.xml
var example_default$20 = "data:text/plain;charset=utf-8,xml";

//#endregion
export { example_default as css, example_default$1 as eot, example_default$2 as gif, example_default$3 as htm, example_default$4 as html, example_default$5 as jpeg, example_default$6 as jpg, example_default$7 as js, example_default$8 as json, example_default$9 as mjs, example_default$10 as otf, example_default$11 as pdf, example_default$12 as png, example_default$13 as sfnt, example_default$14 as svg, example_default$15 as ttf, example_default$16 as wasm, example_default$17 as webp, example_default$18 as woff, example_default$19 as woff2, example_default$20 as xml };
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,22 +1,22 @@
 var example_default = "data:text/css;charset=utf-8,css";
-var example_default2 = "data:application/vnd.ms-fontobject,eot";
-var example_default3 = "data:image/gif,gif";
-var example_default4 = "data:text/html;charset=utf-8,htm";
-var example_default5 = "data:text/html;charset=utf-8,html";
-var example_default6 = "data:image/jpeg,jpeg";
-var example_default7 = "data:image/jpeg,jpg";
-var example_default8 = "data:text/javascript;charset=utf-8,js";
-var example_default9 = "data:application/json;charset=utf-8,json";
-var example_default10 = "data:text/javascript;charset=utf-8,mjs";
-var example_default11 = "data:font/otf,otf";
-var example_default12 = "data:application/pdf,pdf";
-var example_default13 = "data:image/png,png";
-var example_default14 = "data:font/sfnt,sfnt";
-var example_default15 = "data:image/svg+xml,svg";
-var example_default16 = "data:font/ttf,ttf";
-var example_default17 = "data:application/wasm,wasm";
-var example_default18 = "data:image/webp,webp";
-var example_default19 = "data:font/woff,woff";
-var example_default20 = "data:font/woff2,woff2";
-var example_default21 = "data:text/xml;charset=utf-8,xml";
-export {example_default as css, example_default2 as eot, example_default3 as gif, example_default4 as htm, example_default5 as html, example_default6 as jpeg, example_default7 as jpg, example_default8 as js, example_default9 as json, example_default10 as mjs, example_default11 as otf, example_default12 as pdf, example_default13 as png, example_default14 as sfnt, example_default15 as svg, example_default16 as ttf, example_default17 as wasm, example_default18 as webp, example_default19 as woff, example_default20 as woff2, example_default21 as xml};
+var example_default$1 = "data:application/vnd.ms-fontobject,eot";
+var example_default$2 = "data:image/gif,gif";
+var example_default$3 = "data:text/html;charset=utf-8,htm";
+var example_default$4 = "data:text/html;charset=utf-8,html";
+var example_default$5 = "data:image/jpeg,jpeg";
+var example_default$6 = "data:image/jpeg,jpg";
+var example_default$7 = "data:text/javascript;charset=utf-8,js";
+var example_default$8 = "data:application/json;charset=utf-8,json";
+var example_default$9 = "data:text/javascript;charset=utf-8,mjs";
+var example_default$10 = "data:font/otf,otf";
+var example_default$11 = "data:application/pdf,pdf";
+var example_default$12 = "data:image/png,png";
+var example_default$13 = "data:font/sfnt,sfnt";
+var example_default$14 = "data:image/svg+xml,svg";
+var example_default$15 = "data:font/ttf,ttf";
+var example_default$16 = "data:application/wasm,wasm";
+var example_default$17 = "data:image/webp,webp";
+var example_default$18 = "data:font/woff,woff";
+var example_default$19 = "data:font/woff2,woff2";
+var example_default$20 = "data:text/plain;charset=utf-8,xml";
+export {example_default as css, example_default$1 as eot, example_default$2 as gif, example_default$3 as htm, example_default$4 as html, example_default$5 as jpeg, example_default$6 as jpg, example_default$7 as js, example_default$8 as json, example_default$9 as mjs, example_default$10 as otf, example_default$11 as pdf, example_default$12 as png, example_default$13 as sfnt, example_default$14 as svg, example_default$15 as ttf, example_default$16 as wasm, example_default$17 as webp, example_default$18 as woff, example_default$19 as woff2, example_default$20 as xml};

```