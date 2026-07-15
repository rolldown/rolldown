use std::path::Path;

use rolldown::BundleOutput;
use rolldown_common::{Output, StrOrBytes};
use rolldown_error::{BuildDiagnostic, DiagnosticOptions, Severity};
use serde::Serialize;
use xxhash_rust::xxh3::Xxh3;

const DIGEST_SCHEMA: &str = "rolldown-link-baseline-digest-v4";
const CAPTURE_MODEL: &str = "single-standard-generate-run-with-pre-generate-observer";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DigestSet {
  pub capture_model: &'static str,
  pub outcome: &'static str,
  pub output_digest: String,
  pub pre_generate_diagnostic_digest: String,
  pub final_diagnostic_digest: String,
  pub observation_set_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiagnosticDescriptor {
  pub severity: String,
  pub kind: String,
  pub id: Option<PathDescriptor>,
  pub plugin: Option<String>,
  pub exporter: Option<PathDescriptor>,
  pub ids: Option<Vec<PathDescriptor>>,
  pub rendered: String,
  pub file: Option<PathDescriptor>,
  pub line: Option<usize>,
  pub column: Option<usize>,
  pub utf16_offset: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PathDescriptor {
  /// `cwd`, `cwd-relative`, and `literal` are separate hash frames.
  pub category: &'static str,
  /// Platform separators are normalized only when they cannot be filename characters.
  pub value: String,
}

pub struct FramedHasher {
  hasher: Xxh3,
}

impl FramedHasher {
  pub fn new(domain: &str) -> Self {
    let mut value = Self { hasher: Xxh3::default() };
    value.str(DIGEST_SCHEMA);
    value.str(domain);
    value
  }

  pub fn bytes(&mut self, value: &[u8]) {
    self.hasher.update(&(value.len() as u64).to_le_bytes());
    self.hasher.update(value);
  }

  pub fn str(&mut self, value: &str) {
    self.bytes(value.as_bytes());
  }

  pub fn bool(&mut self, value: bool) {
    self.bytes(&[u8::from(value)]);
  }

  pub fn usize(&mut self, value: usize) {
    self.bytes(&(value as u64).to_le_bytes());
  }

  pub fn option_str(&mut self, value: Option<&str>) {
    match value {
      Some(value) => {
        self.bool(true);
        self.str(value);
      }
      None => self.bool(false),
    }
  }

  pub fn finish(&self) -> String {
    format!("{:032x}", self.hasher.digest128())
  }
}

pub fn digest_capture(
  output: &BundleOutput,
  pre_generate_diagnostics: &[DiagnosticDescriptor],
  cwd: &Path,
) -> DigestSet {
  let output_digest = digest_output(output, cwd);
  let pre_generate_diagnostic_digest = digest_descriptors(pre_generate_diagnostics);
  let final_diagnostic_digest = digest_diagnostics(&output.warnings, cwd);
  let observation_set_digest = digest_observation_set(
    "ok",
    &output_digest,
    &pre_generate_diagnostic_digest,
    &final_diagnostic_digest,
  );
  DigestSet {
    capture_model: CAPTURE_MODEL,
    outcome: "ok",
    output_digest,
    pre_generate_diagnostic_digest,
    final_diagnostic_digest,
    observation_set_digest,
  }
}

pub fn digest_failure(
  errors: &[BuildDiagnostic],
  pre_generate_diagnostics: &[DiagnosticDescriptor],
  cwd: &Path,
) -> DigestSet {
  let mut no_output = FramedHasher::new("bundle-output");
  no_output.str("error");
  let output_digest = no_output.finish();
  let pre_generate_diagnostic_digest = digest_descriptors(pre_generate_diagnostics);
  let final_diagnostic_digest = digest_diagnostics(errors, cwd);
  let observation_set_digest = digest_observation_set(
    "error",
    &output_digest,
    &pre_generate_diagnostic_digest,
    &final_diagnostic_digest,
  );
  DigestSet {
    capture_model: CAPTURE_MODEL,
    outcome: "error",
    output_digest,
    pre_generate_diagnostic_digest,
    final_diagnostic_digest,
    observation_set_digest,
  }
}

fn digest_observation_set(
  outcome: &str,
  output_digest: &str,
  pre_generate_diagnostic_digest: &str,
  final_diagnostic_digest: &str,
) -> String {
  let mut digest = FramedHasher::new("observation-set");
  digest.str(CAPTURE_MODEL);
  digest.str(outcome);
  digest.str(output_digest);
  digest.str(pre_generate_diagnostic_digest);
  digest.str(final_diagnostic_digest);
  digest.finish()
}

pub fn digest_diagnostics(diagnostics: &[BuildDiagnostic], cwd: &Path) -> String {
  let descriptors = describe_diagnostics(diagnostics, cwd);
  digest_descriptors(&descriptors)
}

pub fn digest_descriptors(descriptors: &[DiagnosticDescriptor]) -> String {
  let mut digest = FramedHasher::new("diagnostics");
  digest.usize(descriptors.len());
  for diagnostic in descriptors {
    digest.str("diagnostic");
    digest.str(&diagnostic.severity);
    digest.str(&diagnostic.kind);
    digest_option_path(&mut digest, diagnostic.id.as_ref());
    digest.option_str(diagnostic.plugin.as_deref());
    digest_option_path(&mut digest, diagnostic.exporter.as_ref());
    match &diagnostic.ids {
      Some(ids) => {
        digest.bool(true);
        digest.usize(ids.len());
        for id in ids {
          digest_path(&mut digest, id);
        }
      }
      None => digest.bool(false),
    }
    digest.str(&diagnostic.rendered);
    match &diagnostic.file {
      Some(file) => {
        digest.bool(true);
        digest_path(&mut digest, file);
        digest.usize(diagnostic.line.unwrap_or_default());
        digest.usize(diagnostic.column.unwrap_or_default());
        digest.usize(diagnostic.utf16_offset.unwrap_or_default());
      }
      None => digest.bool(false),
    }
  }
  digest.finish()
}

pub fn describe_diagnostics(
  diagnostics: &[BuildDiagnostic],
  cwd: &Path,
) -> Vec<DiagnosticDescriptor> {
  let options = DiagnosticOptions { cwd: cwd.to_path_buf() };
  diagnostics
    .iter()
    .map(|diagnostic| {
      let rendered = diagnostic.to_diagnostic_with(&options);
      let location = rendered.get_primary_location();
      DiagnosticDescriptor {
        severity: match diagnostic.severity() {
          Severity::Info => "info",
          Severity::Error => "error",
          Severity::Warning => "warning",
        }
        .to_string(),
        kind: diagnostic.kind().to_string(),
        id: diagnostic.id().map(|value| encode_path(&value, cwd)),
        plugin: diagnostic.plugin(),
        exporter: diagnostic.exporter().map(|value| encode_path(&value, cwd)),
        ids: diagnostic
          .ids()
          .map(|ids| ids.into_iter().map(|value| encode_path(&value, cwd)).collect()),
        rendered: rendered.convert_to_string(false),
        file: location.as_ref().map(|(file, ..)| encode_path(file, cwd)),
        line: location.as_ref().map(|(_, line, ..)| *line),
        column: location.as_ref().map(|(_, _, column, _)| *column),
        utf16_offset: location.map(|(_, _, _, offset)| offset),
      }
    })
    .collect()
}

pub fn digest_output(output: &BundleOutput, cwd: &Path) -> String {
  let mut digest = FramedHasher::new("bundle-output");
  digest.usize(output.assets.len());
  for asset in &output.assets {
    match asset {
      Output::Chunk(chunk) => {
        digest.str("chunk");
        digest.str(&chunk.name);
        digest.bool(chunk.is_entry);
        digest.bool(chunk.is_dynamic_entry);
        digest_option_path(
          &mut digest,
          chunk.facade_module_id.as_ref().map(|id| encode_path(id, cwd)).as_ref(),
        );
        digest_paths(&mut digest, chunk.module_ids.iter().map(|id| encode_path(id, cwd)));
        digest_strings(&mut digest, chunk.exports.iter().map(ToString::to_string));
        digest_path(&mut digest, &encode_path(&chunk.filename, cwd));
        digest.usize(chunk.modules.keys.len());
        for id in &chunk.modules.keys {
          digest_path(&mut digest, &encode_path(id, cwd));
        }
        digest.usize(chunk.modules.values.len());
        for module in &chunk.modules.values {
          digest.usize(module.exec_order as usize);
          digest_strings(&mut digest, module.rendered_exports.iter().map(ToString::to_string));
          digest.option_str(module.code().as_deref());
        }
        digest_paths(&mut digest, chunk.imports.iter().map(|value| encode_path(value, cwd)));
        digest_paths(
          &mut digest,
          chunk.dynamic_imports.iter().map(|value| encode_path(value, cwd)),
        );
        digest.str(&chunk.code);
        match &chunk.map {
          Some(map) => {
            digest.bool(true);
            digest.str(&map.to_json_string());
          }
          None => digest.bool(false),
        }
        digest_option_path(
          &mut digest,
          chunk.sourcemap_filename.as_ref().map(|value| encode_path(value, cwd)).as_ref(),
        );
        digest_path(&mut digest, &encode_path(&chunk.preliminary_filename, cwd));
      }
      Output::Asset(asset) => {
        digest.str("asset");
        digest_strings(&mut digest, asset.names.iter().map(String::as_str));
        digest_paths(
          &mut digest,
          asset.original_file_names.iter().map(|value| encode_path(value, cwd)),
        );
        digest_path(&mut digest, &encode_path(&asset.filename, cwd));
        match &asset.source {
          StrOrBytes::Str(source) => {
            digest.str("str");
            digest.bytes(source.as_bytes());
          }
          StrOrBytes::Bytes(source) => {
            digest.str("bytes");
            digest.bytes(source);
          }
        }
      }
    }
  }
  digest.finish()
}

fn digest_strings<T: AsRef<str>>(
  digest: &mut FramedHasher,
  values: impl ExactSizeIterator<Item = T>,
) {
  digest.usize(values.len());
  for value in values {
    digest.str(value.as_ref());
  }
}

fn digest_paths(digest: &mut FramedHasher, values: impl ExactSizeIterator<Item = PathDescriptor>) {
  digest.usize(values.len());
  for value in values {
    digest_path(digest, &value);
  }
}

fn digest_option_path(digest: &mut FramedHasher, value: Option<&PathDescriptor>) {
  match value {
    Some(value) => {
      digest.bool(true);
      digest_path(digest, value);
    }
    None => digest.bool(false),
  }
}

fn digest_path(digest: &mut FramedHasher, value: &PathDescriptor) {
  digest.str(value.category);
  digest.str(&value.value);
}

fn encode_path(value: &str, cwd: &Path) -> PathDescriptor {
  let value = normalize_platform_separators(value);
  let cwd = normalize_platform_separators(&cwd.to_string_lossy());
  if cwd.is_empty() {
    return PathDescriptor { category: "literal", value };
  }
  if value == cwd {
    return PathDescriptor { category: "cwd", value: String::new() };
  }
  if let Some(relative) = value.strip_prefix(&format!("{cwd}/")) {
    PathDescriptor { category: "cwd-relative", value: relative.to_string() }
  } else {
    PathDescriptor { category: "literal", value }
  }
}

fn normalize_platform_separators(value: &str) -> String {
  if std::path::MAIN_SEPARATOR == '\\' { value.replace('\\', "/") } else { value.to_string() }
}
