//! Checks GitHub Releases for a newer version. This is the only network access the app
//! makes, and it only happens when the user chooses "Check for Updates…".

use std::cmp::Ordering;
use std::time::Duration;

use gtk4::{self as gtk, gio, glib};
use serde::Deserialize;

use crate::UPDATE_REPOSITORY;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
}

pub fn check(parent: Option<gtk::Window>) {
    glib::spawn_future_local(async move {
        let result = gio::spawn_blocking(latest_release)
            .await
            .unwrap_or_else(|_| Err("The update check stopped unexpectedly.".into()));

        let release = match result {
            Ok(Some(release)) => release,
            Ok(None) => {
                return alert(
                    &parent,
                    "No Releases Found",
                    "There are no published releases of mdviewer yet.",
                );
            }
            Err(error) => return alert(&parent, "Couldn’t Check for Updates", &error),
        };

        let latest = release.tag_name.trim_start_matches(['v', 'V']).to_string();
        if compare_versions(&latest, CURRENT_VERSION) != Ordering::Greater {
            let detail = format!("mdviewer {CURRENT_VERSION} is the latest version.");
            return alert(&parent, "You’re Up to Date", &detail);
        }

        let dialog = gtk::AlertDialog::builder()
            .message(format!("mdviewer {latest} Is Available"))
            .detail(format!(
                "You have version {CURRENT_VERSION}. Download the new version from GitHub?"
            ))
            .buttons(["Later", "Open Release Page"])
            .cancel_button(0)
            .default_button(1)
            .modal(true)
            .build();
        let choice = dialog.choose_future(parent.as_ref()).await;
        // Only ever send the user to github.com.
        if choice == Ok(1) && is_github(&release.html_url) {
            gtk::UriLauncher::new(&release.html_url).launch(
                parent.as_ref(),
                gio::Cancellable::NONE,
                |_| {},
            );
        }
    });
}

fn latest_release() -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{UPDATE_REPOSITORY}/releases/latest");
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    match agent
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .call()
    {
        Ok(mut response) => {
            let body = response
                .body_mut()
                .read_to_string()
                .map_err(|e| e.to_string())?;
            serde_json::from_str(&body)
                .map(Some)
                .map_err(|e| e.to_string())
        }
        Err(ureq::Error::StatusCode(404)) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn is_github(url: &str) -> bool {
    glib::Uri::parse(url, glib::UriFlags::NONE)
        .is_ok_and(|uri| uri.scheme() == "https" && uri.host().as_deref() == Some("github.com"))
}

/// Compares versions the way Finder sorts names: runs of digits compare as numbers.
fn compare_versions(a: &str, b: &str) -> Ordering {
    fn runs(s: &str) -> Vec<(bool, &str)> {
        let mut out = Vec::new();
        let mut start = 0;
        let chars: Vec<(usize, char)> = s.char_indices().collect();
        for (i, &(at, c)) in chars.iter().enumerate() {
            let next_differs = chars
                .get(i + 1)
                .is_none_or(|&(_, n)| n.is_ascii_digit() != c.is_ascii_digit());
            if next_differs {
                let end = at + c.len_utf8();
                out.push((c.is_ascii_digit(), &s[start..end]));
                start = end;
            }
        }
        out
    }
    for (x, y) in runs(a).into_iter().zip(runs(b)) {
        let order = match (x, y) {
            ((true, x), (true, y)) => {
                let (x, y) = (x.trim_start_matches('0'), y.trim_start_matches('0'));
                x.len().cmp(&y.len()).then(x.cmp(y))
            }
            ((_, x), (_, y)) => x.cmp(y),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    runs(a).len().cmp(&runs(b).len())
}

fn alert(parent: &Option<gtk::Window>, message: &str, detail: &str) {
    gtk::AlertDialog::builder()
        .message(message)
        .detail(detail)
        .modal(true)
        .build()
        .show(parent.as_ref());
}

#[cfg(test)]
mod tests {
    use super::*;
    use Ordering::*;

    #[test]
    fn versions_compare_numerically() {
        assert_eq!(compare_versions("0.10.0", "0.9.0"), Greater);
        assert_eq!(compare_versions("1.0.0", "1.0.0"), Equal);
        assert_eq!(compare_versions("0.1.0", "0.1.1"), Less);
        assert_eq!(compare_versions("1.0.1", "1.0"), Greater);
        assert_eq!(compare_versions("2.0", "10.0"), Less);
    }

    #[test]
    fn only_github_release_pages_open() {
        assert!(is_github(
            "https://github.com/adjmx/mdviewer/releases/tag/v0.2.0"
        ));
        assert!(!is_github("https://github.com.evil.example/x"));
        assert!(!is_github("http://github.com/x"));
        assert!(!is_github("file:///etc/passwd"));
    }
}
