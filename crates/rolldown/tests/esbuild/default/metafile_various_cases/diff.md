# Reason
1. not support copy loader
# Diff
## /out/file-NVISQQTV.file
### esbuild
```js
file
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/file-NVISQQTV.file
+++ rolldown	
@@ -1,1 +0,0 @@
-file;

```
## /out/copy-O3Y5SCJE.copy
### esbuild
```js
copy
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/copy-O3Y5SCJE.copy
+++ rolldown	
@@ -1,1 +0,0 @@
-copy;

```
## /out/entry.js
### esbuild
```js
import {
  __commonJS,
  __require
} from "./chunk-MQN2VSL5.js";

// project/cjs.js
var require_cjs = __commonJS({
  "project/cjs.js"(exports, module) {
    module.exports = 4;
  }
});

// project/entry.js
import a from "extern-esm";

// project/esm.js
var esm_default = 1;

// <data:application/json,2>
var json_2_default = 2;

// project/file.file
var file_default = "./file-NVISQQTV.file";

// project/entry.js
import e from "./copy-O3Y5SCJE.copy";
console.log(
  a,
  esm_default,
  json_2_default,
  file_default,
  e,
  __require("extern-cjs"),
  require_cjs(),
  import("./dynamic-Q2DWDUFV.js")
);
var exported;
export {
  exported
};
```
### rolldown
```js
import default$1, { file_default } from "./copy.js";
import a from "extern-esm";



//#region esm.js
var esm_default = 1;
//#endregion

//#region <data:application/json,2>
var json_2_default = 2;
//#endregion

//#region cjs.js
var require_cjs = __commonJS({ "cjs.js"(exports, module) {
	module.exports = 4;
} });
//#endregion

//#region entry.js
console.log(a, esm_default, json_2_default, file_default, default$1, __require("extern-cjs"), require_cjs(), import("./dynamic.js"));
let exported;
//#endregion

export { exported };
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,14 +1,12 @@
-import {__commonJS, __require} from "./chunk-MQN2VSL5.js";
+import default$1, {file_default} from "./copy.js";
+import a from "extern-esm";
+var esm_default = 1;
+var json_2_default = 2;
 var require_cjs = __commonJS({
-    "project/cjs.js"(exports, module) {
+    "cjs.js"(exports, module) {
         module.exports = 4;
     }
 });
-import a from "extern-esm";
-var esm_default = 1;
-var json_2_default = 2;
-var file_default = "./file-NVISQQTV.file";
-import e from "./copy-O3Y5SCJE.copy";
-console.log(a, esm_default, json_2_default, file_default, e, __require("extern-cjs"), require_cjs(), import("./dynamic-Q2DWDUFV.js"));
+console.log(a, esm_default, json_2_default, file_default, default$1, __require("extern-cjs"), require_cjs(), import("./dynamic.js"));
 var exported;
 export {exported};

```
## /out/dynamic-Q2DWDUFV.js
### esbuild
```js
import "./chunk-MQN2VSL5.js";

// project/dynamic.js
var dynamic_default = 5;
export {
  dynamic_default as default
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/dynamic-Q2DWDUFV.js
+++ rolldown	
@@ -1,3 +0,0 @@
-import "./chunk-MQN2VSL5.js";
-var dynamic_default = 5;
-export {dynamic_default as default};

```
## /out/chunk-MQN2VSL5.js
### esbuild
```js
export {
  __require,
  __commonJS
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/chunk-MQN2VSL5.js
+++ rolldown	
@@ -1,4 +0,0 @@
-export {
-  __require,
-  __commonJS
-};
\ No newline at end of file

```
## /out/entry.css
### esbuild
```js
@import "extern.css";

/* project/entry.css */
a {
  background: url(data:image/svg+xml,<svg/>);
}
b {
  background: url("./file-NVISQQTV.file");
}
c {
  background: url("./copy-O3Y5SCJE.copy");
}
d {
  background: url(extern.png);
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.css
+++ rolldown	
@@ -1,15 +0,0 @@
-@import "extern.css";
-
-/* project/entry.css */
-a {
-  background: url(data:image/svg+xml,<svg/>);
-}
-b {
-  background: url("./file-NVISQQTV.file");
-}
-c {
-  background: url("./copy-O3Y5SCJE.copy");
-}
-d {
-  background: url(extern.png);
-}
\ No newline at end of file

```
## metafile.json
### esbuild
```js
{
  "inputs": {
    "project/esm.js": {
      "bytes": 16,
      "imports": [],
      "format": "esm"
    },
    "<data:application/json,2>": {
      "bytes": 1,
      "imports": []
    },
    "project/file.file": {
      "bytes": 4,
      "imports": []
    },
    "project/copy.copy": {
      "bytes": 4,
      "imports": []
    },
    "project/cjs.js": {
      "bytes": 18,
      "imports": [],
      "format": "cjs"
    },
    "project/dynamic.js": {
      "bytes": 16,
      "imports": [],
      "format": "esm"
    },
    "project/entry.js": {
      "bytes": 333,
      "imports": [
        {
          "path": "extern-esm",
          "kind": "import-statement",
          "external": true
        },
        {
          "path": "project/esm.js",
          "kind": "import-statement",
          "original": "./esm"
        },
        {
          "path": "<data:application/json,2>",
          "kind": "import-statement",
          "original": "data:application/json,2"
        },
        {
          "path": "project/file.file",
          "kind": "import-statement",
          "original": "./file.file"
        },
        {
          "path": "project/copy.copy",
          "kind": "import-statement",
          "original": "./copy.copy"
        },
        {
          "path": "extern-cjs",
          "kind": "require-call",
          "external": true
        },
        {
          "path": "project/cjs.js",
          "kind": "require-call",
          "original": "./cjs"
        },
        {
          "path": "project/dynamic.js",
          "kind": "dynamic-import",
          "original": "./dynamic"
        }
      ],
      "format": "esm"
    },
    "project/inline.svg": {
      "bytes": 6,
      "imports": []
    },
    "project/entry.css": {
      "bytes": 180,
      "imports": [
        {
          "path": "extern.css",
          "kind": "import-rule",
          "external": true
        },
        {
          "path": "project/inline.svg",
          "kind": "url-token",
          "original": "inline.svg"
        },
        {
          "path": "project/file.file",
          "kind": "url-token",
          "original": "file.file"
        },
        {
          "path": "project/copy.copy",
          "kind": "url-token",
          "original": "copy.copy"
        },
        {
          "path": "extern.png",
          "kind": "url-token",
          "external": true
        }
      ]
    }
  },
  "outputs": {
    "out/file-NVISQQTV.file": {
      "imports": [],
      "exports": [],
      "inputs": {
        "project/file.file": {
          "bytesInOutput": 4
        }
      },
      "bytes": 4
    },
    "out/copy-O3Y5SCJE.copy": {
      "imports": [],
      "exports": [],
      "inputs": {
        "project/copy.copy": {
          "bytesInOutput": 4
        }
      },
      "bytes": 4
    },
    "out/entry.js": {
      "imports": [
        {
          "path": "out/chunk-MQN2VSL5.js",
          "kind": "import-statement"
        },
        {
          "path": "extern-esm",
          "kind": "import-statement",
          "external": true
        },
        {
          "path": "out/file-NVISQQTV.file",
          "kind": "file-loader"
        },
        {
          "path": "out/copy-O3Y5SCJE.copy",
          "kind": "import-statement"
        },
        {
          "path": "extern-cjs",
          "kind": "require-call",
          "external": true
        },
        {
          "path": "out/dynamic-Q2DWDUFV.js",
          "kind": "dynamic-import"
        }
      ],
      "exports": [
        "exported"
      ],
      "entryPoint": "project/entry.js",
      "inputs": {
        "project/cjs.js": {
          "bytesInOutput": 101
        },
        "project/entry.js": {
          "bytesInOutput": 233
        },
        "project/esm.js": {
          "bytesInOutput": 21
        },
        "<data:application/json,2>": {
          "bytesInOutput": 24
        },
        "project/file.file": {
          "bytesInOutput": 43
        }
      },
      "bytes": 642
    },
    "out/dynamic-Q2DWDUFV.js": {
      "imports": [
        {
          "path": "out/chunk-MQN2VSL5.js",
          "kind": "import-statement"
        }
      ],
      "exports": [
        "default"
      ],
      "entryPoint": "project/dynamic.js",
      "inputs": {
        "project/dynamic.js": {
          "bytesInOutput": 25
        }
      },
      "bytes": 119
    },
    "out/chunk-MQN2VSL5.js": {
      "imports": [],
      "exports": [
        "__commonJS",
        "__require"
      ],
      "inputs": {},
      "bytes": 38
    },
    "out/entry.css": {
      "imports": [
        {
          "path": "extern.css",
          "kind": "import-rule",
          "external": true
        },
        {
          "path": "data:image/svg+xml,<svg/>",
          "kind": "url-token"
        },
        {
          "path": "out/file-NVISQQTV.file",
          "kind": "url-token"
        },
        {
          "path": "out/copy-O3Y5SCJE.copy",
          "kind": "url-token"
        },
        {
          "path": "extern.png",
          "kind": "url-token",
          "external": true
        }
      ],
      "entryPoint": "project/entry.css",
      "inputs": {
        "project/entry.css": {
          "bytesInOutput": 187
        }
      },
      "bytes": 234
    }
  }
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	metafile.json
+++ rolldown	
@@ -1,245 +0,0 @@
-{
-  "inputs": {
-    "project/esm.js": {
-      "bytes": 16,
-      "imports": [],
-      "format": "esm"
-    },
-    "<data:application/json,2>": {
-      "bytes": 1,
-      "imports": []
-    },
-    "project/file.file": {
-      "bytes": 4,
-      "imports": []
-    },
-    "project/copy.copy": {
-      "bytes": 4,
-      "imports": []
-    },
-    "project/cjs.js": {
-      "bytes": 18,
-      "imports": [],
-      "format": "cjs"
-    },
-    "project/dynamic.js": {
-      "bytes": 16,
-      "imports": [],
-      "format": "esm"
-    },
-    "project/entry.js": {
-      "bytes": 333,
-      "imports": [
-        {
-          "path": "extern-esm",
-          "kind": "import-statement",
-          "external": true
-        },
-        {
-          "path": "project/esm.js",
-          "kind": "import-statement",
-          "original": "./esm"
-        },
-        {
-          "path": "<data:application/json,2>",
-          "kind": "import-statement",
-          "original": "data:application/json,2"
-        },
-        {
-          "path": "project/file.file",
-          "kind": "import-statement",
-          "original": "./file.file"
-        },
-        {
-          "path": "project/copy.copy",
-          "kind": "import-statement",
-          "original": "./copy.copy"
-        },
-        {
-          "path": "extern-cjs",
-          "kind": "require-call",
-          "external": true
-        },
-        {
-          "path": "project/cjs.js",
-          "kind": "require-call",
-          "original": "./cjs"
-        },
-        {
-          "path": "project/dynamic.js",
-          "kind": "dynamic-import",
-          "original": "./dynamic"
-        }
-      ],
-      "format": "esm"
-    },
-    "project/inline.svg": {
-      "bytes": 6,
-      "imports": []
-    },
-    "project/entry.css": {
-      "bytes": 180,
-      "imports": [
-        {
-          "path": "extern.css",
-          "kind": "import-rule",
-          "external": true
-        },
-        {
-          "path": "project/inline.svg",
-          "kind": "url-token",
-          "original": "inline.svg"
-        },
-        {
-          "path": "project/file.file",
-          "kind": "url-token",
-          "original": "file.file"
-        },
-        {
-          "path": "project/copy.copy",
-          "kind": "url-token",
-          "original": "copy.copy"
-        },
-        {
-          "path": "extern.png",
-          "kind": "url-token",
-          "external": true
-        }
-      ]
-    }
-  },
-  "outputs": {
-    "out/file-NVISQQTV.file": {
-      "imports": [],
-      "exports": [],
-      "inputs": {
-        "project/file.file": {
-          "bytesInOutput": 4
-        }
-      },
-      "bytes": 4
-    },
-    "out/copy-O3Y5SCJE.copy": {
-      "imports": [],
-      "exports": [],
-      "inputs": {
-        "project/copy.copy": {
-          "bytesInOutput": 4
-        }
-      },
-      "bytes": 4
-    },
-    "out/entry.js": {
-      "imports": [
-        {
-          "path": "out/chunk-MQN2VSL5.js",
-          "kind": "import-statement"
-        },
-        {
-          "path": "extern-esm",
-          "kind": "import-statement",
-          "external": true
-        },
-        {
-          "path": "out/file-NVISQQTV.file",
-          "kind": "file-loader"
-        },
-        {
-          "path": "out/copy-O3Y5SCJE.copy",
-          "kind": "import-statement"
-        },
-        {
-          "path": "extern-cjs",
-          "kind": "require-call",
-          "external": true
-        },
-        {
-          "path": "out/dynamic-Q2DWDUFV.js",
-          "kind": "dynamic-import"
-        }
-      ],
-      "exports": [
-        "exported"
-      ],
-      "entryPoint": "project/entry.js",
-      "inputs": {
-        "project/cjs.js": {
-          "bytesInOutput": 101
-        },
-        "project/entry.js": {
-          "bytesInOutput": 233
-        },
-        "project/esm.js": {
-          "bytesInOutput": 21
-        },
-        "<data:application/json,2>": {
-          "bytesInOutput": 24
-        },
-        "project/file.file": {
-          "bytesInOutput": 43
-        }
-      },
-      "bytes": 642
-    },
-    "out/dynamic-Q2DWDUFV.js": {
-      "imports": [
-        {
-          "path": "out/chunk-MQN2VSL5.js",
-          "kind": "import-statement"
-        }
-      ],
-      "exports": [
-        "default"
-      ],
-      "entryPoint": "project/dynamic.js",
-      "inputs": {
-        "project/dynamic.js": {
-          "bytesInOutput": 25
-        }
-      },
-      "bytes": 119
-    },
-    "out/chunk-MQN2VSL5.js": {
-      "imports": [],
-      "exports": [
-        "__commonJS",
-        "__require"
-      ],
-      "inputs": {},
-      "bytes": 38
-    },
-    "out/entry.css": {
-      "imports": [
-        {
-          "path": "extern.css",
-          "kind": "import-rule",
-          "external": true
-        },
-        {
-          "path": "data:image/svg+xml,<svg/>",
-          "kind": "url-token"
-        },
-        {
-          "path": "out/file-NVISQQTV.file",
-          "kind": "url-token"
-        },
-        {
-          "path": "out/copy-O3Y5SCJE.copy",
-          "kind": "url-token"
-        },
-        {
-          "path": "extern.png",
-          "kind": "url-token",
-          "external": true
-        }
-      ],
-      "entryPoint": "project/entry.css",
-      "inputs": {
-        "project/entry.css": {
-          "bytesInOutput": 187
-        }
-      },
-      "bytes": 234
-    }
-  }
-}
\ No newline at end of file

```