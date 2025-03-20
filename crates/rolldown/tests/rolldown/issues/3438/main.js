import { clear as repro1_clear } from "./repro1.js"
import { clear as repro2_clear, clear$1 as repro2_clear$1 } from "./repro2.js"
import { clear as repro3_clear, clear$1 as repro3_clear$1, clear$2 as repro3_clear$2 } from "./repro3.js"

export default [
  repro1_clear,
  repro2_clear,
  repro2_clear$1,
  repro3_clear,
  repro3_clear$1,
  repro3_clear$2,
  () => import("./repro1.js").then(console.log),
  () => import("./repro2.js").then(console.log),
  () => import("./repro3.js").then(console.log),
]
