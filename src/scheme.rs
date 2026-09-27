//! The `mdviewer-file:` scheme: serves files next to the open document (relative images,
//! media) to the web view. Only files inside the document's folder are readable.

use std::path::{Path, PathBuf};

use gtk4::{gio, glib, prelude::*};

pub const SCHEME: &str = "mdviewer-file";
const PREFIX: &str = "mdviewer-file://local";

/// The page's base URI for a document folder, e.g. `mdviewer-file://local/home/me/notes/`.
pub fn base_uri(directory: &Path) -> String {
    let file_uri = gio::File::for_path(directory).uri();
    let path = file_uri.strip_prefix("file://").unwrap_or(&file_uri);
    let slash = if path.ends_with('/') { "" } else { "/" };
    format!("{PREFIX}{path}{slash}")
}

/// The `file://` URI an `mdviewer-file:` URI names, without query or fragment.
pub fn file_uri(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix(PREFIX)?;
    if !rest.starts_with('/') {
        return None;
    }
    let end = rest.find(['?', '#']).unwrap_or(rest.len());
    Some(format!("file://{}", &rest[..end]))
}

/// The local path an `mdviewer-file:` URI names.
pub fn file_path(uri: &str) -> Option<PathBuf> {
    gio::File::for_uri(&file_uri(uri)?).path()
}

/// `path`, with symlinks resolved, if it lies strictly inside `root`.
pub fn contained(path: &Path, root: &Path) -> Option<PathBuf> {
    let root = root.canonicalize().ok()?;
    let path = path.canonicalize().ok()?;
    (path != root && path.starts_with(&root)).then_some(path)
}

/// Answers one scheme request, given the folder of the document that asked.
pub fn serve(request: &webkit6::URISchemeRequest, root: Option<&Path>) {
    let found = request
        .uri()
        .and_then(|uri| file_path(&uri))
        .zip(root)
        .and_then(|(path, root)| contained(&path, root))
        .filter(|path| path.is_file())
        .and_then(|path| std::fs::read(&path).ok().map(|data| (path, data)));

    let Some((path, data)) = found else {
        let mut error = glib::Error::new(gio::IOErrorEnum::NotFound, "not found");
        request.finish_error(&mut error);
        return;
    };

    let (content_type, _) = gio::content_type_guess(Some(&path), Some(data.as_slice()));
    let mime = gio::content_type_get_mime_type(&content_type);
    let length = data.len() as i64;
    let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from_owned(data));
    request.finish(&stream, length, mime.as_deref());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_uri_escapes_and_ends_in_a_slash() {
        assert_eq!(
            base_uri(Path::new("/home/me/my notes")),
            "mdviewer-file://local/home/me/my%20notes/"
        );
    }

    #[test]
    fn file_path_round_trips_and_drops_the_fragment() {
        let uri = format!(
            "{}img/a%20b.png#x",
            base_uri(Path::new("/home/me/my notes"))
        );
        assert_eq!(
            file_path(&uri),
            Some(PathBuf::from("/home/me/my notes/img/a b.png"))
        );
    }

    #[test]
    fn other_schemes_and_hosts_are_not_files() {
        assert_eq!(file_uri("https://example.com/x"), None);
        assert_eq!(file_uri("mdviewer-file://elsewhere/x"), None);
        assert_eq!(file_uri("mdviewer-file://localhost/x"), None);
    }

    #[test]
    fn containment_refuses_escapes() {
        let dir = std::env::temp_dir().join(format!("mdviewer-test-{}", std::process::id()));
        let docs = dir.join("docs");
        std::fs::create_dir_all(docs.join("img")).unwrap();
        std::fs::write(docs.join("img/a.png"), b"x").unwrap();
        std::fs::write(dir.join("secret"), b"x").unwrap();
        std::os::unix::fs::symlink(dir.join("secret"), docs.join("link")).unwrap();

        assert!(contained(&docs.join("img/a.png"), &docs).is_some());
        assert!(contained(&docs.join("../secret"), &docs).is_none());
        assert!(
            contained(&docs.join("link"), &docs).is_none(),
            "symlink out of the folder"
        );
        assert!(contained(&docs, &docs).is_none(), "the folder itself");
        assert!(contained(&docs.join("missing.png"), &docs).is_none());

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
