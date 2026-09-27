# Backlog

Ideas for future polish, mirrored from the macOS app's backlog so the two stay in step. Keep
everything offline: documents must never touch the network.

## 1. Remember the last opened file

Reopen the most recent `.md` file at launch when mdviewer starts with no files.

- GTK has no window restoration, so this is always the app's job here (on macOS the system may
  restore windows itself).
- Store the last path in a small state file under `~/.local/state/mdviewer/` (`-dev` for
  debug builds). If the file has moved, skip it silently.

## 2. Show the file path of the open document

- Show the path as a header-bar subtitle, with `~` for the home folder.
- Click to open the folder in the file manager, or right-click to copy the path.

## 3. Open / close / save dialogs for light editing

- New, Save, Save As, and an "unsaved changes" prompt on close.
- UI idea: a plain `GtkTextView` editor next to the rendered preview (a `GtkPaned`), or a
  toggle between Edit and Preview.
- Auto-refresh must respect unsaved edits. If the file changes on disk while there are
  unsaved changes, ask instead of overwriting.

## Linux-specific

- **Sandboxing the app process.** The web process is already in WebKit's bubblewrap sandbox
  (enabled through `packaging/apparmor/mdviewer`). A Flatpak would be the closest match to
  the macOS App Sandbox for the app process too.
- **Fail gracefully without the AppArmor profile.** If the profile isn't loaded, WebKit
  aborts the whole app on the first document. Detecting that up front and showing an
  explanation would beat a crash.

## Other

- Decide on a license (possibly MIT). See the README.
