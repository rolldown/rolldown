import { CrossFileGood, CrossFileBad } from './cross-file'
const enum SameFileGood {
	STR = 'str 1',
	NUM = 123,
}
const enum SameFileBad {
	PROTO = '__proto__',
	CONSTRUCTOR = 'constructor',
	PROTOTYPE = 'prototype',
}
class Foo {
	[100] = 100;
	'200' = 200;
	['300'] = 300;
	[SameFileGood.STR] = SameFileGood.STR;
	[SameFileGood.NUM] = SameFileGood.NUM;
	[CrossFileGood.STR] = CrossFileGood.STR;
	[CrossFileGood.NUM] = CrossFileGood.NUM;
}
shouldNotBeComputed(
	class {
		[100] = 100;
		'200' = 200;
		['300'] = 300;
		[SameFileGood.STR] = SameFileGood.STR;
		[SameFileGood.NUM] = SameFileGood.NUM;
		[CrossFileGood.STR] = CrossFileGood.STR;
		[CrossFileGood.NUM] = CrossFileGood.NUM;
	},
	{
		[100]: 100,
		'200': 200,
		['300']: 300,
		[SameFileGood.STR]: SameFileGood.STR,
		[SameFileGood.NUM]: SameFileGood.NUM,
		[CrossFileGood.STR]: CrossFileGood.STR,
		[CrossFileGood.NUM]: CrossFileGood.NUM,
	},
)
mustBeComputed(
	{ [SameFileBad.PROTO]: null },
	{ [CrossFileBad.PROTO]: null },
	class { [SameFileBad.CONSTRUCTOR]() {} },
	class { [CrossFileBad.CONSTRUCTOR]() {} },
	class { static [SameFileBad.PROTOTYPE]() {} },
	class { static [CrossFileBad.PROTOTYPE]() {} },
)