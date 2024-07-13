class Remove1 {}
class Remove2 { *[Symbol.iterator]() {} }
class Remove3 { *[Symbol['iterator']]() {} }

class Keep1 { *[Symbol.iterator]() {} [keep] }
class Keep2 { [keep]; *[Symbol.iterator]() {} }
class Keep3 { *[Symbol.wtf]() {} }
