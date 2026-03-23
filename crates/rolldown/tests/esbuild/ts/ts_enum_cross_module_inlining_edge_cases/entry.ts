import { Renamed } from './re-export-renamed'
import { ns } from './re-export-ns'
import { A, B, C } from './enums'

// 1. Renamed re-export: should inline
console.log(Renamed.x)

// 2. Multiple enums with same member name: should inline correctly
console.log(A.x, B.x)

// 3. Mixed value types in same enum
console.log(A.x, A.y)

// 4. Namespace re-export: ns.A.x
console.log(ns.A.x)

// 5. Const enum: should inline and remove declaration
console.log(C.x)
