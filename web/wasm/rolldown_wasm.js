import * as wasm from './rolldown_wasm_bg.wasm'
import { __wbg_set_wasm } from './rolldown_wasm_bg.js'
__wbg_set_wasm(wasm)
export * from './rolldown_wasm_bg.js'
