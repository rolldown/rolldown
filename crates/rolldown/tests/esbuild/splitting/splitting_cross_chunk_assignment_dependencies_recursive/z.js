import { setY } from './y'
let _z
export function setZ(v) { _z = v }
export function setZ2(v) { setY(v); _z = v }