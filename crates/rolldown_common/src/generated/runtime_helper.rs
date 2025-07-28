// Auto-generated code, DO NOT EDIT DIRECTLY!
// To edit this generated file you have to edit `tasks/generator/src/generators/runtime_helper.rs`

use bitflags::bitflags;
bitflags! {
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
  pub struct RuntimeHelper: u32 {
    const Create = 1 << 0;
    const DefProp = 1 << 1;
    const Name = 1 << 2;
    const GetOwnPropDesc = 1 << 3;
    const GetOwnPropNames = 1 << 4;
    const GetProtoOf = 1 << 5;
    const HasOwnProp = 1 << 6;
    const Esm = 1 << 7;
    const EsmMin = 1 << 8;
    const CommonJs = 1 << 9;
    const CommonJsMin = 1 << 10;
    const Export = 1 << 11;
    const CopyProps = 1 << 12;
    const ReExport = 1 << 13;
    const ToEsm = 1 << 14;
    const ToCommonJs = 1 << 15;
    const ToBinaryNode = 1 << 16;
    const ToBinary = 1 << 17;
    const ToDynamicImportEsm = 1 << 18;
    const Require = 1 << 19;
  }
}

pub const RUNTIME_HELPER_NAMES: [&str; 20] = [
  "__create",
  "__defProp",
  "__name",
  "__getOwnPropDesc",
  "__getOwnPropNames",
  "__getProtoOf",
  "__hasOwnProp",
  "__esm",
  "__esmMin",
  "__commonJS",
  "__commonJSMin",
  "__export",
  "__copyProps",
  "__reExport",
  "__toESM",
  "__toCommonJS",
  "__toBinaryNode",
  "__toBinary",
  "__toDynamicImportESM",
  "__require",
];
