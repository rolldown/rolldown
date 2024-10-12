export namespace foo { export enum x { y, yy = y } }
export namespace foo { export enum x { z = y + 1 } }

declare let y: any, z: any
export namespace foo.x {
	console.log(y, z)
	console.log(x.y, x.z)
}