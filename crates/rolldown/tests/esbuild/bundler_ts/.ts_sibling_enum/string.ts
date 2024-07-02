export enum x { y = 'a', yy = y }
export enum x { z = y }

declare let y: any, z: any
export namespace x { console.log(y, z) }
console.log(x.y, x.z)