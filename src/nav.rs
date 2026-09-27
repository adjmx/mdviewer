//! What the web view may navigate to. The reader never touches the network.

use crate::scheme;

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    Block,
    /// Refuse in the viewer, and hand this URI to the user's default app instead.
    OpenExternally(String),
}

/// Decides a navigation to `uri`.
///
/// `clicked` is true only for links the user activated (including ones asking for a new
/// window); everything else — the page load itself, meta refresh, redirects, frames — is
/// something the document started on its own.
pub fn decide(uri: &str, clicked: bool, current: Option<&str>) -> Decision {
    if !clicked {
        // The only loads the app starts are `about:` and a document's base URI, which is
        // always a folder. Refusing other local files stops a document from navigating the
        // viewer to a page of its own (e.g. a sibling .html via meta refresh).
        let is_folder = uri.starts_with(scheme::SCHEME) && without_fragment(uri).ends_with('/');
        return if uri.starts_with("about:") || is_folder {
            Decision::Allow
        } else {
            Decision::Block
        };
    }

    // In-page anchors (#section) scroll within the document.
    if uri.contains('#')
        && current.is_some_and(|current| without_fragment(current) == without_fragment(uri))
    {
        return Decision::Allow;
    }

    // Links the user clicks open in their default app (e.g. the browser), not in the reader.
    if let Some(file_uri) = scheme::file_uri(uri) {
        return Decision::OpenExternally(file_uri);
    }
    let scheme = uri.split_once(':').map(|(s, _)| s.to_ascii_lowercase());
    match scheme.as_deref() {
        Some("http" | "https" | "mailto") => Decision::OpenExternally(uri.to_string()),
        _ => Decision::Block,
    }
}

fn without_fragment(uri: &str) -> &str {
    uri.split_once('#').map_or(uri, |(before, _)| before)
}

#[cfg(test)]
mod tests {
    use super::*;
    use Decision::*;

    const PAGE: &str = "mdviewer-file://local/home/me/notes/";

    #[test]
    fn the_app_can_load_the_page_and_about_blank() {
        assert_eq!(decide(PAGE, false, None), Allow);
        assert_eq!(decide("about:blank", false, Some(PAGE)), Allow);
    }

    #[test]
    fn documents_cannot_navigate_on_their_own() {
        assert_eq!(decide("https://example.com/", false, Some(PAGE)), Block);
        assert_eq!(decide("file:///etc/passwd", false, Some(PAGE)), Block);
        assert_eq!(
            decide(&format!("{PAGE}other.html"), false, Some(PAGE)),
            Block
        );
        assert_eq!(decide("javascript:alert(1)", false, Some(PAGE)), Block);
    }

    #[test]
    fn anchors_scroll_in_place() {
        assert_eq!(decide(&format!("{PAGE}#building"), true, Some(PAGE)), Allow);
    }

    #[test]
    fn clicked_links_leave_the_reader() {
        assert_eq!(
            decide("https://example.com/", true, Some(PAGE)),
            OpenExternally("https://example.com/".into())
        );
        assert_eq!(
            decide("MAILTO:a@b.c", true, Some(PAGE)),
            OpenExternally("MAILTO:a@b.c".into())
        );
        assert_eq!(
            decide(&format!("{PAGE}other.md#top"), true, Some(PAGE)),
            OpenExternally("file:///home/me/notes/other.md".into())
        );
    }

    #[test]
    fn clicked_links_to_other_schemes_do_nothing() {
        assert_eq!(decide("file:///etc/passwd", true, Some(PAGE)), Block);
        assert_eq!(decide("javascript:alert(1)", true, Some(PAGE)), Block);
        assert_eq!(decide("ftp://example.com/", true, Some(PAGE)), Block);
    }
}
