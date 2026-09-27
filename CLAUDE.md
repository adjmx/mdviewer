# mdviewer (Linux) — notes for Claude

GTK4 + WebKitGTK 6.0 + Rust. The Linux twin of `macos-node/mdviewer` (SwiftUI + WKWebView),
kept at feature parity with it. When one app gains a feature, the other's BACKLOG should get it.

## Build and verify

```sh
make check        # cargo fmt --check, clippy -D warnings, unit tests
make dev FILE=x.md
make deb          # cargo-deb → target/debian/
make install      # sudo apt install the .deb (the user runs this; there is no passwordless sudo)
```

Not Tauri, on purpose: a Tauri app's own UI is JavaScript in the same web view, so document
JavaScript can't be switched off. Here the app is native GTK and the web view holds only the
document.

## Layout

- `src/render.rs` — comrak options + the page shell (CSP lives here).
- `src/nav.rs` — the navigation policy, as a pure function. Unit-tested; keep it that way.
- `src/scheme.rs` — `mdviewer-file:` serving + folder containment (symlinks resolved).
- `src/offline.rs` — content-blocking rules (same JSON as the macOS app's). Fail closed.
- `src/window.rs` — one window per document, the web view setup, file watching, actions.
- `resources/` — **shared with the macOS app** (`style.css`, `hl-*.css`, `highlight.min.js`,
  `viewer.js`). Copy changes across both ways; `viewer.js` talks to
  `window.webkit.messageHandlers.copyCode`, which both WebKits provide.

## Things that are easy to get wrong

- **The AppArmor profile is load-bearing.** WebKitGTK 6.0 always sandboxes the web process
  with bubblewrap; on Ubuntu 23.10+ that needs `packaging/apparmor/mdviewer` (installed and
  loaded by the .deb's postinst). Without it the app aborts on the first document with
  `bwrap: setting up uid map: Permission denied` / `Failed to fully launch dbus-proxy`.
- **Commands run by Claude don't see that failure.** They inherit the `claude-desktop`
  AppArmor profile, which allows userns. Test the way the desktop launches apps with
  `aa-exec -p unconfined -- /usr/bin/mdviewer file.md` (this is how the crash was reproduced).
- **comrak is pinned `~0.55`.** GFM's `tagfilter` is deprecated in 0.55 and removed in 0.56.
  Don't bump past 0.55 without replacing it.
- **Dark mode** mostly comes from the GTK theme (Ubuntu flips `Yaru` ↔ `Yaru-dark` over
  XSETTINGS); `theme.rs` also mirrors GNOME's `color-scheme` for desktops that only flip that.
  To see light mode in a dev run without touching the desktop: `GTK_THEME=Yaru make dev`.
- **Debug builds are a different app** (`uk.fizx.mdviewer.dev`). A dev run started while the
  installed app is open still gets its own window instead of handing the file over.
- **Offline test:** open a document that points remote images, iframes, scripts and a meta
  refresh at a local HTTP logger (e.g. `127.0.0.1:8765`); the logger must see nothing.
- **GUI verification on this box:** python-xlib XTEST + ImageMagick `import`, as for the other
  Linux apps. A click on the header bar can swallow the next shortcut; click the page first.
