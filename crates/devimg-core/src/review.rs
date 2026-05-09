use std::collections::BTreeMap;

use crate::manifest::{Manifest, ManifestOutput};
use crate::quality::manifest_quality_warnings;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestReviewOptions {
    pub asset_path_prefix: String,
    pub top_limit: usize,
}

impl Default for ManifestReviewOptions {
    fn default() -> Self {
        Self {
            asset_path_prefix: String::new(),
            top_limit: 8,
        }
    }
}

pub fn render_manifest_review(manifest: &Manifest, options: &ManifestReviewOptions) -> String {
    let groups = grouped_outputs(manifest);
    let warnings = review_warnings(manifest);
    let source_count = groups.len();
    let top_limit = options.top_limit.max(1);

    let mut out = String::new();
    out.push_str("<!doctype html>\n");
    out.push_str("<html lang=\"en\">\n");
    out.push_str("<head>\n");
    out.push_str("  <meta charset=\"utf-8\">\n");
    out.push_str("  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    out.push_str("  <title>DevImg Review</title>\n");
    out.push_str("  <style>\n");
    out.push_str(STYLES);
    out.push_str("  </style>\n");
    out.push_str("</head>\n");
    out.push_str("<body>\n");
    out.push_str("  <main class=\"page\">\n");
    out.push_str("    <header class=\"hero\">\n");
    out.push_str("      <p class=\"eyebrow\">DevImg visual review</p>\n");
    out.push_str("      <h1>Generated image variants</h1>\n");
    out.push_str("      <dl class=\"meta-grid\">\n");
    push_stat(&mut out, "Sources", &source_count.to_string());
    push_stat(&mut out, "Variants", &manifest.outputs.len().to_string());
    push_stat(
        &mut out,
        "Source bytes",
        &format_bytes(manifest.source_bytes_total()),
    );
    push_stat(
        &mut out,
        "Output bytes",
        &format_bytes(manifest.output_bytes_total()),
    );
    push_stat(&mut out, "Budget status", "not evaluated from manifest");
    push_stat(&mut out, "Config hash", &short_hash(&manifest.config_hash));
    out.push_str("      </dl>\n");
    out.push_str("    </header>\n");

    out.push_str("    <section class=\"panel\">\n");
    out.push_str("      <div class=\"section-heading\">\n");
    out.push_str("        <h2>Manifest</h2>\n");
    out.push_str("      </div>\n");
    out.push_str("      <div class=\"manifest-grid\">\n");
    push_key_value(&mut out, "Generated at", &manifest.generated_at);
    push_key_value(&mut out, "Config path", &manifest.config_path);
    push_key_value(&mut out, "Config hash", &manifest.config_hash);
    push_key_value(
        &mut out,
        "Budget status",
        "not evaluated; run devimg check --config <path>",
    );
    out.push_str("      </div>\n");
    out.push_str("    </section>\n");

    out.push_str("    <section class=\"panel\">\n");
    out.push_str("      <div class=\"section-heading\">\n");
    out.push_str("        <h2>Review signals</h2>\n");
    out.push_str("      </div>\n");
    if warnings.is_empty() {
        out.push_str("      <p class=\"empty-state\">No manifest-only warnings.</p>\n");
    } else {
        out.push_str("      <ul class=\"signal-list\">\n");
        for warning in &warnings {
            out.push_str("        <li>");
            push_escaped(&mut out, warning);
            out.push_str("</li>\n");
        }
        out.push_str("      </ul>\n");
    }
    out.push_str("    </section>\n");

    out.push_str("    <section class=\"panel split-panel\">\n");
    push_top_sources(&mut out, &groups, top_limit);
    push_top_outputs(&mut out, &manifest.outputs, top_limit);
    out.push_str("    </section>\n");

    for (source_path, outputs) in &groups {
        push_source_group(&mut out, source_path, outputs, options);
    }

    out.push_str("  </main>\n");
    out.push_str("</body>\n");
    out.push_str("</html>\n");
    out
}

fn grouped_outputs(manifest: &Manifest) -> BTreeMap<&str, Vec<&ManifestOutput>> {
    let mut groups = BTreeMap::<&str, Vec<&ManifestOutput>>::new();
    for output in &manifest.outputs {
        groups
            .entry(output.source_path.as_str())
            .or_default()
            .push(output);
    }
    for outputs in groups.values_mut() {
        outputs.sort_by(|left, right| {
            left.preset
                .cmp(&right.preset)
                .then(left.width.cmp(&right.width))
                .then(left.height.cmp(&right.height))
                .then(left.format.cmp(&right.format))
                .then(left.output_path.cmp(&right.output_path))
        });
    }
    groups
}

fn review_warnings(manifest: &Manifest) -> Vec<String> {
    if manifest.outputs.is_empty() {
        return vec!["Manifest has no generated variants.".to_string()];
    }
    manifest_quality_warnings(manifest)
}

fn push_source_group(
    out: &mut String,
    source_path: &str,
    outputs: &[&ManifestOutput],
    options: &ManifestReviewOptions,
) {
    let first = outputs[0];
    let source_href = asset_href(source_path, &options.asset_path_prefix);
    let variant_total: u64 = outputs.iter().map(|output| output.bytes).sum();

    out.push_str("    <section class=\"source-group\">\n");
    out.push_str("      <div class=\"source-intro\">\n");
    out.push_str("        <div>\n");
    out.push_str("          <p class=\"eyebrow\">Source</p>\n");
    out.push_str("          <h2>");
    push_escaped(out, source_path);
    out.push_str("</h2>\n");
    out.push_str("        </div>\n");
    out.push_str("        <dl class=\"source-stats\">\n");
    push_stat(
        out,
        "Dimensions",
        &format!("{}x{}", first.source_width, first.source_height),
    );
    push_stat(out, "Source bytes", &format_bytes(first.source_bytes));
    push_stat(out, "Variant bytes", &format_bytes(variant_total));
    push_stat(out, "Variants", &outputs.len().to_string());
    out.push_str("        </dl>\n");
    out.push_str("      </div>\n");

    out.push_str("      <div class=\"source-preview\">\n");
    push_image(
        out,
        &source_href,
        &format!("Source image: {source_path}"),
        first.source_width,
        first.source_height,
    );
    out.push_str("      </div>\n");

    out.push_str("      <div class=\"variant-grid\">\n");
    for output in outputs {
        push_variant_card(out, output, options);
    }
    out.push_str("      </div>\n");
    out.push_str("    </section>\n");
}

fn push_variant_card(out: &mut String, output: &ManifestOutput, options: &ManifestReviewOptions) {
    let output_href = asset_href(&output.output_path, &options.asset_path_prefix);
    out.push_str("        <article class=\"variant-card\">\n");
    out.push_str("          <div class=\"variant-image\">\n");
    push_image(
        out,
        &output_href,
        &format!(
            "{} {} {}x{}",
            output.preset, output.format, output.width, output.height
        ),
        output.width,
        output.height,
    );
    out.push_str("          </div>\n");
    out.push_str("          <div class=\"variant-body\">\n");
    out.push_str("            <div class=\"variant-title\">\n");
    out.push_str("              <strong>");
    push_escaped(out, &output.preset);
    out.push_str("</strong>\n");
    out.push_str("              <span>");
    push_escaped(out, &output.format);
    out.push_str("</span>\n");
    out.push_str("            </div>\n");
    out.push_str("            <dl class=\"variant-meta\">\n");
    push_key_value(out, "Size", &format!("{}x{}", output.width, output.height));
    push_key_value(out, "Fit", &output.fit);
    push_key_value(out, "Bytes", &format_bytes(output.bytes));
    push_key_value(out, "Hash", &short_hash(&output.hash));
    out.push_str("            </dl>\n");
    out.push_str("            <a class=\"path-link\" href=\"");
    push_attr_escaped(out, &output_href);
    out.push_str("\">");
    push_escaped(out, &output.output_path);
    out.push_str("</a>\n");
    out.push_str("          </div>\n");
    out.push_str("        </article>\n");
}

fn push_top_sources(
    out: &mut String,
    groups: &BTreeMap<&str, Vec<&ManifestOutput>>,
    top_limit: usize,
) {
    let mut sources: Vec<(&str, u64)> = groups
        .iter()
        .map(|(source, outputs)| (*source, outputs[0].source_bytes))
        .collect();
    sources.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));

    out.push_str("      <div>\n");
    out.push_str("        <h2>Largest sources</h2>\n");
    push_ranked_list(out, sources.into_iter().take(top_limit));
    out.push_str("      </div>\n");
}

fn push_top_outputs(out: &mut String, outputs: &[ManifestOutput], top_limit: usize) {
    let mut ranked: Vec<(&str, u64)> = outputs
        .iter()
        .map(|output| (output.output_path.as_str(), output.bytes))
        .collect();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));

    out.push_str("      <div>\n");
    out.push_str("        <h2>Largest outputs</h2>\n");
    push_ranked_list(out, ranked.into_iter().take(top_limit));
    out.push_str("      </div>\n");
}

fn push_ranked_list<'a>(out: &mut String, items: impl Iterator<Item = (&'a str, u64)>) {
    let mut count = 0;
    out.push_str("        <ol class=\"ranked-list\">\n");
    for (path, bytes) in items {
        count += 1;
        out.push_str("          <li><span>");
        push_escaped(out, path);
        out.push_str("</span><strong>");
        push_escaped(out, &format_bytes(bytes));
        out.push_str("</strong></li>\n");
    }
    if count == 0 {
        out.push_str("          <li><span>No entries.</span><strong>0 B</strong></li>\n");
    }
    out.push_str("        </ol>\n");
}

fn push_stat(out: &mut String, label: &str, value: &str) {
    out.push_str("        <div>\n");
    out.push_str("          <dt>");
    push_escaped(out, label);
    out.push_str("</dt>\n");
    out.push_str("          <dd>");
    push_escaped(out, value);
    out.push_str("</dd>\n");
    out.push_str("        </div>\n");
}

fn push_key_value(out: &mut String, label: &str, value: &str) {
    out.push_str("              <div><dt>");
    push_escaped(out, label);
    out.push_str("</dt><dd>");
    push_escaped(out, value);
    out.push_str("</dd></div>\n");
}

fn push_image(out: &mut String, href: &str, alt: &str, width: u32, height: u32) {
    out.push_str("            <a href=\"");
    push_attr_escaped(out, href);
    out.push_str("\"><img src=\"");
    push_attr_escaped(out, href);
    out.push_str("\" alt=\"");
    push_attr_escaped(out, alt);
    out.push_str("\" width=\"");
    out.push_str(&width.to_string());
    out.push_str("\" height=\"");
    out.push_str(&height.to_string());
    out.push_str("\" loading=\"lazy\" decoding=\"async\"></a>\n");
}

fn asset_href(path: &str, prefix: &str) -> String {
    let path = path.replace('\\', "/");
    let prefix = prefix.replace('\\', "/");
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() {
        path
    } else if path.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}/{}", path.trim_start_matches('/'))
    }
}

fn short_hash(hash: &str) -> String {
    hash.split_once(':')
        .map(|(_, value)| value)
        .unwrap_or(hash)
        .chars()
        .take(12)
        .collect()
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= MB {
        format!("{:.2} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes / KB)
    } else {
        format!("{} B", bytes as u64)
    }
}

fn push_escaped(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
}

fn push_attr_escaped(out: &mut String, value: &str) {
    push_escaped(out, value);
}

const STYLES: &str = r#":root {
    color-scheme: light;
    --paper: #f7f4ee;
    --ink: #1c211f;
    --muted: #646b67;
    --line: #d8d1c5;
    --panel: #fffdf8;
    --accent: #0e6f68;
    --accent-ink: #06443f;
    --warn: #9a5a00;
    --danger: #9d2f2f;
    --shadow: 0 16px 38px rgba(44, 38, 28, 0.08);
  }

  * {
    box-sizing: border-box;
  }

  body {
    margin: 0;
    background:
      linear-gradient(90deg, rgba(28, 33, 31, 0.035) 1px, transparent 1px),
      linear-gradient(180deg, rgba(28, 33, 31, 0.035) 1px, transparent 1px),
      var(--paper);
    background-size: 36px 36px;
    color: var(--ink);
    font-family: ui-sans-serif, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  .page {
    width: min(1440px, calc(100% - 40px));
    margin: 0 auto;
    padding: 40px 0 56px;
  }

  .hero,
  .panel,
  .source-group {
    background: rgba(255, 253, 248, 0.94);
    border: 1px solid var(--line);
    box-shadow: var(--shadow);
  }

  .hero {
    padding: 32px;
    border-radius: 6px;
  }

  .eyebrow {
    margin: 0 0 10px;
    color: var(--accent-ink);
    font-size: 12px;
    font-weight: 800;
    letter-spacing: 0;
    text-transform: uppercase;
  }

  h1,
  h2 {
    margin: 0;
    letter-spacing: 0;
  }

  h1 {
    max-width: 920px;
    font-size: 72px;
    line-height: 0.95;
  }

  h2 {
    font-size: 20px;
  }

  .meta-grid,
  .source-stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 12px;
    margin: 28px 0 0;
  }

  .meta-grid div,
  .source-stats div {
    min-width: 0;
    border-top: 3px solid var(--accent);
    padding-top: 10px;
  }

  dt {
    color: var(--muted);
    font-size: 12px;
    font-weight: 700;
    text-transform: uppercase;
  }

  dd {
    margin: 5px 0 0;
    min-width: 0;
    overflow-wrap: anywhere;
    font-weight: 750;
  }

  .panel,
  .source-group {
    margin-top: 18px;
    border-radius: 6px;
    padding: 24px;
  }

  .section-heading {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 18px;
    margin-bottom: 18px;
  }

  .manifest-grid,
  .variant-meta {
    display: grid;
    gap: 10px;
  }

  .manifest-grid {
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  }

  .manifest-grid div,
  .variant-meta div {
    min-width: 0;
  }

  .signal-list,
  .ranked-list {
    margin: 0;
    padding-left: 20px;
  }

  .signal-list li {
    margin: 8px 0;
    color: var(--warn);
    font-weight: 700;
    overflow-wrap: anywhere;
  }

  .empty-state {
    margin: 0;
    color: var(--muted);
  }

  .split-panel {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 28px;
  }

  .ranked-list {
    display: grid;
    gap: 8px;
    margin-top: 14px;
  }

  .ranked-list li {
    display: flex;
    justify-content: space-between;
    gap: 18px;
    border-bottom: 1px solid var(--line);
    padding-bottom: 8px;
  }

  .ranked-list span {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .ranked-list strong {
    flex: 0 0 auto;
  }

  .source-group {
    display: grid;
    grid-template-columns: minmax(280px, 380px) minmax(0, 1fr);
    gap: 22px;
  }

  .source-intro {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(260px, 0.8fr);
    gap: 24px;
  }

  .source-intro h2 {
    overflow-wrap: anywhere;
  }

  .source-preview,
  .variant-image {
    display: grid;
    place-items: center;
    min-height: 180px;
    overflow: hidden;
    border: 1px solid var(--line);
    background: #ece7dc;
  }

  .source-preview {
    align-self: start;
  }

  img {
    display: block;
    width: 100%;
    height: auto;
    max-height: 520px;
    object-fit: contain;
  }

  .source-preview img {
    max-height: 420px;
  }

  .variant-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 14px;
  }

  .variant-card {
    min-width: 0;
    overflow: hidden;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--panel);
  }

  .variant-body {
    display: grid;
    gap: 12px;
    padding: 14px;
  }

  .variant-title {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .variant-title span {
    border: 1px solid var(--line);
    border-radius: 999px;
    padding: 3px 8px;
    color: var(--accent-ink);
    font-size: 12px;
    font-weight: 800;
    text-transform: uppercase;
  }

  .path-link {
    color: var(--accent-ink);
    overflow-wrap: anywhere;
    font-size: 12px;
    font-weight: 700;
    text-decoration-thickness: 2px;
    text-underline-offset: 3px;
  }

  @media (max-width: 860px) {
    .page {
      width: min(100% - 24px, 720px);
      padding-top: 20px;
    }

    .hero,
    .panel,
    .source-group {
      padding: 18px;
    }

    h1 {
      font-size: 42px;
    }

    .split-panel,
    .source-group,
    .source-intro {
      grid-template-columns: 1fr;
    }
  }
"#;

#[cfg(test)]
mod tests {
    use super::{render_manifest_review, ManifestReviewOptions};
    use crate::manifest::{Manifest, ManifestOutput};

    #[test]
    fn review_html_escapes_manifest_text() {
        let manifest = Manifest {
            version: 1,
            generated_at: "unix:1".to_string(),
            config_path: "dev\"img.toml".to_string(),
            config_hash: "blake3:<config>&\"".to_string(),
            outputs: vec![ManifestOutput {
                source_path: "assets/<card>&\".png".to_string(),
                source_hash: "blake3:source".to_string(),
                source_width: 800,
                source_height: 450,
                source_bytes: 123,
                output_path: "public/images/<card>.webp".to_string(),
                preset: "project-card<script>".to_string(),
                fit: "cover".to_string(),
                width: 640,
                height: 360,
                format: "webp".to_string(),
                bytes: 45,
                hash: "blake3:output".to_string(),
                operation_hash: "blake3:operation".to_string(),
            }],
        };

        let html = render_manifest_review(&manifest, &ManifestReviewOptions::default());

        assert!(html.contains("assets/&lt;card&gt;&amp;&quot;.png"));
        assert!(html.contains("project-card&lt;script&gt;"));
        assert!(html.contains("public/images/&lt;card&gt;.webp"));
        assert!(!html.contains("project-card<script>"));
    }

    #[test]
    fn review_html_groups_sources_and_links_assets() {
        let manifest = Manifest {
            version: 1,
            generated_at: "unix:1".to_string(),
            config_path: "devimg.toml".to_string(),
            config_hash: "blake3:config".to_string(),
            outputs: vec![
                output(
                    "assets/card.png",
                    "public/images/card.960.webp",
                    960,
                    540,
                    200,
                ),
                output(
                    "assets/card.png",
                    "public/images/card.640.webp",
                    640,
                    360,
                    100,
                ),
                output(
                    "assets/logo.png",
                    "public/images/logo.256.webp",
                    256,
                    256,
                    50,
                ),
            ],
        };
        let options = ManifestReviewOptions {
            asset_path_prefix: "..".to_string(),
            top_limit: 2,
        };

        let html = render_manifest_review(&manifest, &options);

        assert_eq!(html.matches("class=\"source-group\"").count(), 2);
        assert!(html.contains("Sources</dt>\n          <dd>2</dd>"));
        assert!(html.contains("Variants</dt>\n          <dd>3</dd>"));
        assert!(html.contains("src=\"../public/images/card.640.webp\""));
        assert!(html.contains("href=\"../assets/card.png\""));
        assert!(html.contains("Largest outputs"));
    }

    fn output(source: &str, path: &str, width: u32, height: u32, bytes: u64) -> ManifestOutput {
        ManifestOutput {
            source_path: source.to_string(),
            source_hash: format!("blake3:{source}"),
            source_width: 800,
            source_height: 450,
            source_bytes: 500,
            output_path: path.to_string(),
            preset: "project-card".to_string(),
            fit: "cover".to_string(),
            width,
            height,
            format: "webp".to_string(),
            bytes,
            hash: format!("blake3:{path}"),
            operation_hash: format!("blake3:{path}:operation"),
        }
    }
}
