const ns = await import ('./lib')
eval("ns.a")

import("./lib2").then(res => {})

