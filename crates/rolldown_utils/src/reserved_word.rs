static RESERVED_WORDS: &[&str] = &[
  "break",
  "case",
  "catch",
  "class",
  "const",
  "continue",
  "debugger",
  "default",
  "delete",
  "do",
  "else",
  "enum",
  "export",
  "extends",
  "false",
  "finally",
  "for",
  "function",
  "if",
  "import",
  "in",
  "instanceof",
  "new",
  "null",
  "package",
  "return",
  "super",
  "switch",
  "this",
  "throw",
  "true",
  "try",
  "typeof",
  "var",
  "void",
  "while",
  "with",
];
static RESERVED_WORDS_STRICT: &[&str] = &[
  "await", // in module
  "implements",
  "interface",
  "let",
  "package",
  "private",
  "protected",
  "public",
  "static",
  "yield",
];
static RESERVED_WORDS_STRICT_BIND: &[&str] = &["eval", "arguments"];
static RESERVED_WORDS_ES3: &[&str] = &[
  "abstract",
  "boolean",
  "byte",
  "char",
  "double",
  "final",
  "float",
  "goto",
  "int",
  "long",
  "native",
  "short",
  "synchronized",
  "throws",
  "transient",
  "volatile",
];

pub fn is_reserved_word(s: &str) -> bool {
  RESERVED_WORDS.contains(&s)
    || RESERVED_WORDS_STRICT.contains(&s)
    || RESERVED_WORDS_STRICT_BIND.contains(&s)
    || RESERVED_WORDS_ES3.contains(&s)
}
