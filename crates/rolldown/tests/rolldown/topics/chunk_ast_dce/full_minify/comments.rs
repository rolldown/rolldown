use rolldown::{Bundler, BundlerOptions, InputItem};
use rolldown_common::{
  CommentsOptions, Output, RawCompressOptions, RawMinifyOptions, RawMinifyOptionsDetailed,
};

#[tokio::test(flavor = "multi_thread")]
async fn comment_policy() {
  for minify in [
    RawMinifyOptions::DeadCodeEliminationOnly,
    RawMinifyOptions::Bool(true),
    RawMinifyOptions::Object(RawMinifyOptionsDetailed {
      mangle: None,
      mangle_properties: None,
      compress: Some(RawCompressOptions::default()),
      remove_whitespace: false,
      ascii_only: false,
    }),
  ] {
    for legal in [false, true] {
      for annotation in [false, true] {
        for jsdoc in [false, true] {
          let mut bundler = Bundler::new(BundlerOptions {
            input: Some(vec![InputItem {
              name: Some("comments".into()),
              import: "./comments.js".into(),
            }]),
            cwd: Some(
              concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/rolldown/topics/chunk_ast_dce/full_minify"
              )
              .into(),
            ),
            minify: Some(minify.clone()),
            comments: Some(CommentsOptions { legal, annotation, jsdoc }),
            ..Default::default()
          })
          .unwrap();
          let output = bundler.generate().await.unwrap();
          let chunk = output
            .assets
            .iter()
            .find_map(|asset| match asset {
              Output::Chunk(chunk) => Some(chunk),
              Output::Asset(_) => None,
            })
            .unwrap();
          let code = &chunk.code;
          assert!(!code.contains("ordinary-marker"), "{minify:?}: {code}");
          assert_eq!(code.contains("legal-marker"), legal, "{minify:?}: {code}");
          let whitespace = matches!(minify, RawMinifyOptions::Bool(true));
          assert_eq!(code.contains("jsdoc-marker"), jsdoc && !whitespace, "{minify:?}: {code}");
          assert_eq!(code.contains("@vite-ignore"), annotation, "{minify:?}: {code}");
          assert!(
            !code.contains("externalPureCall"),
            "pure calls must be removed even when annotation comments are hidden: {code}"
          );
        }
      }
    }
  }
}
