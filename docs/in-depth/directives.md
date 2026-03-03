# Directive

JavaScript has a feature called directive, which is used to annotate a part of the code.

Rolldown may not be able to preserve the semantics related to directives, here are the strategies when handling directives.

## `"use strict"`

The `"use strict"` directive is a directive that tells the JavaScript engine to enforce strict mode. Because keeping the top-level `"use strict"` directive semantics is complicated and requires a bigger output size, Rolldown may not keep them.

Since ES modules are always in strict mode, Rolldown does not output any `"use strict"` directive for `output.format: 'es'`. As a side note, this means code that is not in strict mode are forced to be in strict mode for ES module format output.

When `output.format` is not `'es'`, Rolldown will output the `"use strict"` directive for any of the following cases:

- The directive is not in the top-level scope and not inside strict mode scope ([REPL](https://repl.rolldown.rs/#eNptjk0KAyEMha8SsrGF4gGE3mQ24mhxsMmgsR0YvHu1pT+LbpJ8L+G97BjQ7Bhp9pteypgJzZdP6Dr6beUsRQdmOEOo5CQygT0cYZ8IQNXioUiOTtTg7KVmAtXvO7eJuo9HI7n6dsLMKc18J+2YQrxo+cT+2fw+ALMPtibpoXnEcJW1inkjQOB8tV1QbinqJbbRngVbz751s2TFF8H2AIc5VRY=))
- The directive is in the top-level scope and the module is a entry module ([REPL](https://repl.rolldown.rs/#eNptUEtuhDAMvYqVDVCN6Kobuuw12FBwpqmCQx2nTYVy9zGD5qPRbJK8Z7+PshprutU4mjC333F7k+lu+GBGhVWKCFHYjVK99+TmJbDA1/+C/JE+ESyHGar2Nf6kgVF121ZPY6AYPLY+HOvrcv3WNDpVZzSdcMJyMFfdJf9GPC3QE+ZzhQntkLyATTSKCwS7sM4NrD0BMEpiggwvkFVXNFfjOHg/hT9qtaB1x7vcJ5O9wEPe2vNmH5IsSboLBLCB50GJatQ/2MmyXefDFM3+VTM/CEYx5QSMo4I7))
- The directive is in the top-level scope and `output.preserveModules` is enabled ([REPL](https://repl.rolldown.rs/#eNptkE1uhDAMha9iZQNUiK66ocuuewM2FJwpVYip40ypUO5eZxAz1Wg2Sfzz/L14M9a0m5n8iGvzFfLbm/YW12bQsIgBIQhPgxSvnZ/mhVjg83dBfosfCJZphqJ5Dt+xZ1Rd7ur8QD6Qw8bRqbw2ly9VpVWdjKYVjphqc9Ud/FvioQFcLwZGtH10Ajb6QSbysMvKtYKt8wCMEtnDCk+wqiopVWFMzo304xu1Z6fTP+qDyo6/420d5/EUZYnSHiGAJZ57TRSDbqA+sgtjQD7jO43RYWghf3ovpnxdDpPU2VlRrhcMYtIfQpqMFA==))

If you want to append `"use strict"` to all files, you can use the `output.intro` option:

```ts
import { defineConfig } from 'rolldown';

export default defineConfig({
  output: {
    intro: "'use strict';",
  },
});
```

## Other directives

The ECMAScript specification allows implementations to define additional directives. Since those additional directives are not part of the specification, Rolldown does not know the semantics of them. Rolldown assumes that they follow the similar semantics as `"use strict"`. But for the same reason as above, Rolldown may not preserve the top-level directives.

Rolldown will output the directive for any of the following cases:

- The directive is not in the top-level scope ([REPL](https://repl.rolldown.rs/#eNptjt0KwyAMhV8l5MYNig8g7E16I1ZHi02Kxq1QfPfpxn4udpPkOwnn5MCA5sCZJr/rJfeZ0Hx5QNfQ7xsnyTowwwVCISczE9jTGY6RAFTJHlzJwqvqnLyURKDafeM6UvPxaCQVXwdMHOPEd9KOKcxXLZ/YP5vfB2DywZYoLTT1GC6yFTFvBAicVtsE5ZasXmLt7VmwtuxbM4tWfBasD4hqVRg=))
- The directive is in the top-level scope and the module is a entry module ([REPL](https://repl.rolldown.rs/#eNptUM1OwzAMfhUrl7ZolBOXcuQ1eimtM4pSuzgOdKr67riLtiHYJYn9/Sqr865Z3UgDLvVH3N/kmtt8cL2NRYoIfYrK0yOSyql4aWmcZhaF99OM8preELzwBEX9FD9TJ2jqndVSzxQ5YB34WF7J5XNVGWr+6BqVhNvBXXWXFrfF/xoZywm4nJsM6LsUFHyiXkcmyJxyqWBtCUBQkxAs8ACL6TaLt1ThEAb+ptp6+vH4K/4Oknv8yVtb2e056Zy0uYwAnmXqbFH09hV5ue3X+XCbZX+ZWegUo7rtB/Gqh1w=))
- The directive is in the top-level scope and `output.preserveModules` is enabled ([REPL](https://repl.rolldown.rs/#eNptkM9ShDAMxl8l0wvgIJ684NGzb8AFIV1xSoNps7LD8O6mMOw6upe2+fPl+zWLsaZezOB7nKvPkN7e1Le4NJ2GmQSETkKk8RF95Ev20vhhnIgjfFwm5Fd5R7BMI2TVU/iSllHVqavxHflADitHp/zanD8XhVZ1Ppo6suBamqvuoLgl/mOM1IvD5IDzxtGjbcVFsOK7OJCHXZ3PBSyNB2CMwh5meIBZVauaqyeTcz19+0op7XD6ZX6nslP88VsaTuNJ4iSxPkIASzy2msg6XUR5ZCfGgHzGtw0/1JD+vhfXdG2HWZXsrFaujRiiWX8ALR2RKg==))

If you want to append custom directive to all files, you can use the `output.banner` option:

```ts
import { defineConfig } from 'rolldown';

export default defineConfig({
  output: {
    banner: "'use client';",
  },
});
```
