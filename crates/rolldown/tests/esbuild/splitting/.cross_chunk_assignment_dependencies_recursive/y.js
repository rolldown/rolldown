import { setX } from './x'
let _y
export function setY(v) { _y = v }
export function setY2(v) { setX(v); _y = v }