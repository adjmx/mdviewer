# mdviewer (Linux)

A tiny native Linux Markdown reader: GTK4 + WebKitGTK 6.0, written in Rust, rendered with
[comrak](https://github.com/kivikakk/comrak) (GitHub-flavored Markdown).

It is the Linux twin of [macos-node/mdviewer](https://github.com/macos-node/mdviewer): same goal,
same features, same offline guarantees. The web resources (`resources/`: stylesheet,
highlight.js, `viewer.js`) are shared with the macOS app.

## Features

- Open `.md` / `.markdown` files via Open (Ctrl+O), drag-and-drop, or the file manager's
  *Open With* — each document gets its own window
- GFM: tables, task lists, ~~strikethrough~~, autolinks, footnotes
- Auto-refreshes when the file changes on disk (including editors that save by replacing the
  file); Ctrl+R / F5 / toolbar button to reload manually. The scroll position is kept.
- Syntax highlighting (highlight.js, GitHub themes) and a Copy button on code blocks
- Check for Updates… against GitHub Releases (the only network access, on demand)
- Light & dark mode (follows the desktop), Ctrl+plus/minus/0 and Ctrl+scroll zoom, print (Ctrl+P)
- Relative images and links next to the document work
- Heading anchors: `[jump](#building)` scrolls to `## Building`

## Install

```sh
make install     # builds target/debian/mdviewer_<version>-1_<arch>.deb and installs it with apt
```

Or install a downloaded [release](https://github.com/adjmx/mdviewer/releases/latest):
`sudo apt install ./mdviewer_<version>-1_amd64.deb`.

Releasing: bump `version` in `Cargo.toml` (and `Cargo.lock`), commit, then push a `v<version>`
tag. `.github/workflows/release.yml` refuses a tag that doesn't match `Cargo.toml`.

## Building

Needs Rust, GTK 4.14+, WebKitGTK 6.0 and [cargo-deb](https://github.com/kornelski/cargo-deb):

```sh
sudo apt install libgtk-4-dev libwebkitgtk-6.0-dev
cargo install cargo-deb
make dev FILE=README.md   # debug build + run (WebKit sandbox off; see the Makefile)
make check                # fmt, clippy (warnings are errors), unit tests
make deb                  # release .deb in target/debian/
```

Debug builds run as a separate app (`uk.fizx.mdviewer.dev`, cache under `~/.cache/mdviewer-dev`),
so a dev run never hands its files to an installed, already-running mdviewer.

The icons in `packaging/icons/` are the macOS app's 1024 px icon, cropped to the Linux fill
(`make icons`).

## Security notes

Documents never touch the network:

- **WebKit content-blocking rules** — every load except local content (`mdviewer-file:`,
  `data:`, `about:`) is blocked inside the viewer. The rules are the same JSON as the macOS
  app's. If they can't be installed, the document isn't shown.
- **Content Security Policy** — a second, independent block: only local images may load; remote
  images, fonts, frames, forms and `<base>` are refused. DNS prefetching is off.
- **No document JavaScript** — `enable-javascript-markup` is off, so scripts, event handlers and
  `javascript:` URLs in the Markdown never run. Syntax highlighting and copy buttons run as the
  app's own scripts in a separate script world.
- **No automatic navigation** — a document can't navigate the viewer (meta refresh, redirects,
  frames). Links you click open in your default browser or app.
- **Only the document's folder** is served to the page, with symlinks resolved first.
- **Ephemeral web session** — no cookies, cache or storage are written to disk.
- **Sandboxed web process** — WebKitGTK runs the page in a bubblewrap sandbox. The package
  installs `/etc/apparmor.d/mdviewer`, the same one-line `userns` grant Ubuntu ships for
  Epiphany and Geary, because Ubuntu 23.10+ otherwise blocks the sandbox and WebKit aborts.

The app process itself is not sandboxed (there is no equivalent of the macOS App Sandbox): it
runs with your normal file access, and serves the page only the document's folder.

The one exception to "offline" is **Check for Updates…** (menu), which the app (not a document)
runs only when you choose it. It calls `api.github.com/repos/adjmx/mdviewer/releases/latest` and
offers to open the release page on github.com. Nothing is downloaded or installed automatically.

highlight.js (BSD-3-Clause) is bundled in `resources/`.

See [BACKLOG.md](BACKLOG.md) for planned improvements.

## License

No license has been chosen yet, so all rights are reserved for now. An MIT license may be added
later. highlight.js, bundled in `resources/`, is under its own BSD-3-Clause license.
