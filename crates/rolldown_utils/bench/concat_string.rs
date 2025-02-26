use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rolldown_utils::{concat_string, mime::MimeExt};

fn bench_concat_string(c: &mut Criterion) {
  let mut group = c.benchmark_group("concat_string");
  let long1 = "a".repeat(1000);
  let long2 = "b".repeat(1000);
  let base64 = "base64".repeat(1000);

  let mime_ext = MimeExt { mime: "image/png".parse().unwrap(), is_utf8_encoded: true };

  // Mix of String and str
  group.bench_function("mixed_types_concat", |b| {
    let mime_ext_string = mime_ext.to_string();
    b.iter(|| black_box(concat_string!("data:", mime_ext_string, ";base64,", base64)));
  });
  group.bench_function("mixed_types_format", |b| {
    b.iter(|| black_box(format!("data:{mime_ext};base64,{base64}")));
  });

  // Longer strings
  group.bench_function("long_strings_concat", |b| {
    b.iter(|| black_box(concat_string!(long1.as_str(), long2.as_str())));
  });
  group.bench_function("long_strings_format", |b| {
    b.iter(|| black_box(format!("{long1}{long2}")));
  });

  group.finish();
}

criterion_group!(benches, bench_concat_string);
criterion_main!(benches);
