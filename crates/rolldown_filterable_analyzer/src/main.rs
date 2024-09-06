use rolldown_filterable_analyzer::filterable;

fn main() {
  let source = r#"
async function test() {

      throw new Error(
        '"ESM integration proposal for Wasm" is not supported currently. ' +
          'Use vite-plugin-wasm or other community plugins to handle this. ' +
          'Alternatively, you can use `.wasm?init` or `.wasm?url`. ' +
          'See https://vitejs.dev/guide/features.html#webassembly for more details.',
      )
}

  "#;
  filterable(source);
}
