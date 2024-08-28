// We intend to store plain structs and enums in this folder. What 'plain' means is that these structs
// and enums do not have complex logic, and are used to store data. They are not used to perform any
// operations on the data they store or only have simple getters and setters.

pub mod ast_symbols;
pub mod bundle_output;
pub mod bundler_fs;
pub mod generator;
pub mod linking_metadata;
pub mod module_factory;
pub mod namespace_alias;
pub mod oxc_parse_type;
pub mod symbols;
