//! Markdown → HTML, and the page the fragment is shown in.

use comrak::{Options, markdown_to_html};

use crate::scheme;

/// Renders GitHub-flavored Markdown to an HTML fragment.
///
/// Raw HTML is passed through: the web view disables document JavaScript, and the
/// CSP plus the content-blocking rules keep the document off the network.
pub fn html(markdown: &str) -> String {
    let mut options = Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    // GFM's disallowed-raw-HTML rule, as cmark-gfm applies it on macOS. Deprecated in comrak
    // 0.55 and gone in 0.56, which is why Cargo.toml pins `~0.55`.
    #[allow(deprecated)]
    {
        options.extension.tagfilter = true;
    }
    options.extension.footnotes = true;
    // GitHub-style `id`s on headings, so `[link](#section-name)` anchors work.
    options.extension.header_id_prefix = Some(String::new());
    options.parse.smart = true;
    options.render.r#unsafe = true;
    markdown_to_html(markdown, &options)
}

/// Wraps a rendered HTML fragment in a complete, styled page.
pub fn page(body: &str, title: &str) -> String {
    let scheme = scheme::SCHEME;
    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta http-equiv="x-dns-prefetch-control" content="off">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src {scheme}: data:; media-src {scheme}:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'">
<title>{title}</title>
<style>{STYLESHEET}</style>
</head>
<body><article class="markdown-body">
{body}
</article></body>
</html>
"#,
        title = escape(title),
    )
}

// Syntax themes: GitHub light, and GitHub dark in dark mode. The base stylesheet comes last to win.
const STYLESHEET: &str = concat!(
    include_str!("../resources/hl-light.css"),
    "\n@media (prefers-color-scheme: dark) {\n",
    include_str!("../resources/hl-dark.css"),
    "\n}\n",
    include_str!("../resources/style.css"),
);

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_the_gfm_extensions() {
        let out =
            html("| a |\n|---|\n| b |\n\n~~gone~~ www.example.com\n\n- [x] done\n- [ ] todo\n");
        assert!(out.contains("<table>"));
        assert!(out.contains("<del>gone</del>"));
        assert!(out.contains(r#"<a href="http://www.example.com">"#));
        assert!(out.contains(r#"type="checkbox" checked="""#));
    }

    #[test]
    fn footnotes_render() {
        let out = html("Text[^1].\n\n[^1]: The note.\n");
        assert!(out.contains("footnotes"));
        assert!(out.contains("The note."));
    }

    #[test]
    fn headings_get_github_style_ids() {
        let out = html("# Hello World\n\n## Hello World\n\n### Jump to *Building*\n");
        assert!(out.contains(r#"<h1 id="hello-world">"#));
        assert!(out.contains(r#"<h2 id="hello-world-1">"#));
        assert!(out.contains(r#"<h3 id="jump-to-building">"#));
    }

    #[test]
    fn raw_html_passes_but_tagfilter_neuters_script() {
        let out = html("<kbd>K</kbd>\n\n<script>alert(1)</script>\n");
        assert!(out.contains("<kbd>K</kbd>"));
        assert!(!out.contains("<script>"));
    }

    #[test]
    fn fenced_code_keeps_its_language_class() {
        let out = html("```swift\nlet x = 1\n```\n");
        assert!(out.contains(r#"<code class="language-swift">"#));
    }

    #[test]
    fn page_escapes_the_title_and_carries_the_csp() {
        let out = page("<p>x</p>", "<a&b>.md");
        assert!(out.contains("<title>&lt;a&amp;b&gt;.md</title>"));
        assert!(out.contains("default-src 'none'; img-src mdviewer-file: data:"));
    }
}
