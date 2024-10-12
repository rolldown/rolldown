import { CrossFile } from './cross-file'
enum SameFile {
	STR = 'str 1',
	NUM = 123,
}
console.log(`
	SameFile.STR = ${SameFile.STR}
	SameFile.NUM = ${SameFile.NUM}
	CrossFile.STR = ${CrossFile.STR}
	CrossFile.NUM = ${CrossFile.NUM}
`)