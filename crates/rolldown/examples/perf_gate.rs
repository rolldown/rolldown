use std::{fs, path::PathBuf, time::Instant};

use rolldown::{
  Bundler, BundlerOptions, ExperimentalOptions, InputItem, SourceMapType, TreeshakeOptions,
};

struct TempFixture(PathBuf);

impl Drop for TempFixture {
  fn drop(&mut self) {
    let _ = fs::remove_dir_all(&self.0);
  }
}

fn generate_fixture_dir(tag: &str) -> TempFixture {
  let root = std::env::temp_dir().join(format!(
    "rolldown-perf-gate-{}-{tag}",
    std::process::id()
  ));
  let _ = fs::remove_dir_all(&root);
  fs::create_dir_all(&root).expect("create temp fixture dir");

  let mut barrel = String::new();
  let mut entry = String::from("import { m0, m1, m2, m3, m4 } from './barrel.js';\n");
  for i in 0..120 {
    if i == 0 {
      fs::write(
        root.join("mod_0.js"),
        "export const m0 = () => 1;\nexport const unused_0 = () => 0;\n",
      )
      .expect("write mod_0");
    } else {
      let prev = i - 1;
      let full = format!(
        "import {{ m{prev} }} from './mod_{prev}.js';\nexport const m{i} = () => {i} + m{prev}();\nexport const unused_{i} = () => {i} * 10;\n"
      );
      fs::write(root.join(format!("mod_{i}.js")), full).expect("write chained mod");
    }
    barrel.push_str(&format!("export {{ m{i}, unused_{i} }} from './mod_{i}.js';\n"));
  }
  fs::write(root.join("barrel.js"), barrel).expect("write barrel");

  for d in 0..8 {
    let target = d * 10 + 5;
    let dyn_code = format!(
      "import {{ m{target} }} from './barrel.js';\nexport const dynVal{d} = () => m{target}();\n"
    );
    fs::write(root.join(format!("dyn_{d}.js")), dyn_code).expect("write dyn");
    entry.push_str(&format!("export const load_{d} = () => import('./dyn_{d}.js');\n"));
  }
  entry.push_str("export const total = () => m0() + m1() + m2() + m3() + m4();\n");
  fs::write(root.join("entry.js"), entry).expect("write entry");
  TempFixture(root)
}

async fn bench_case(name: &str, rounds: usize, strict_and_lazy: bool) -> (u128, usize) {
  assert!(rounds >= 1, "ROUNDS must be >= 1");
  let dir = generate_fixture_dir(name);
  let cwd = dir.0.clone();
  let mut best_ns = u128::MAX;
  let mut asset_count = 0usize;

  for _ in 0..rounds {
    let mut bundler = Bundler::new(BundlerOptions {
      input: Some(vec![InputItem {
        name: Some("entry".to_string()),
        import: "./entry.js".to_string(),
      }]),
      cwd: Some(cwd.clone()),
      sourcemap: Some(SourceMapType::File),
      treeshake: TreeshakeOptions::Boolean(true),
      strict_execution_order: Some(strict_and_lazy),
      experimental: Some(ExperimentalOptions {
        lazy_barrel: Some(strict_and_lazy),
        ..Default::default()
      }),
      ..Default::default()
    })
    .expect("create bundler");

    let start = Instant::now();
    let out = bundler.generate().await.expect("generate bundle");
    let elapsed = start.elapsed().as_nanos();
    asset_count = out.assets.len();
    assert!(asset_count > 0, "expected generated assets for {name}");
    best_ns = best_ns.min(elapsed);
  }
  println!("RESULT|{name}|{asset_count}|{best_ns}");
  (best_ns, asset_count)
}

#[tokio::main]
async fn main() {
  let rounds: usize = std::env::var("ROUNDS")
    .ok()
    .map(|s| s.parse::<usize>().expect("ROUNDS must be a positive integer"))
    .unwrap_or(5);
  assert!(rounds >= 1, "ROUNDS must be >= 1");

  bench_case("bundle_eager_120", rounds, false).await;
  bench_case("bundle_strict_lazy_barrel_120", rounds, true).await;
}
