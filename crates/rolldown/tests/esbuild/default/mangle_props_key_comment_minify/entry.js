x = class {
    _mangleThis = 1;
    [/* @__KEY__ */ '_mangleThisToo'] = 2;
    '_doNotMangleThis' = 3;
}
x = {
    _mangleThis: 1,
    [/* @__KEY__ */ '_mangleThisToo']: 2,
    '_doNotMangleThis': 3,
}
x._mangleThis = 1
x[/* @__KEY__ */ '_mangleThisToo'] = 2
x['_doNotMangleThis'] = 3
x([
    `${foo}.${/* @__KEY__ */ '_mangleThis'} = bar.${/* @__KEY__ */ '_mangleThisToo'}`,
    `${foo}.${/* @__KEY__ */ 'notMangled'} = bar.${/* @__KEY__ */ 'notMangledEither'}`,
])
