# mdviewer (Linux) — GTK4 + WebKitGTK 6.0. Build deps: libgtk-4-dev libwebkitgtk-6.0-dev, cargo-deb.
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ARCH    := $(shell dpkg --print-architecture)
DEB     := target/debian/mdviewer_$(VERSION)-1_$(ARCH).deb
FILE    ?=

.PHONY: dev check test deb install uninstall icons clean

dev:            ## debug build + run (separate app id and cache from the installed one)
	cargo run -- $(FILE)

check:          ## fmt + clippy (warnings are errors) + unit tests
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test

test:
	cargo test

deb:            ## release build packaged as a .deb
	cargo deb

install: deb    ## install the .deb through apt (asks for sudo)
	sudo apt install --reinstall ./$(DEB)

uninstall:
	sudo apt remove mdviewer

# Linux icons crop Apple's grid margin (926/1024 of the canvas), so the tile fills ~89%
# of the hicolor square. source-1024.png is the macOS app's 1024 icon, never edited.
icons:
	cd packaging/icons && for s in 16 24 32 48 64 128 256 512; do \
	  convert source-1024.png -crop 926x926+49+49 +repage -filter Lanczos -resize $${s}x$${s} mdviewer-$$s.png; \
	done

clean:
	cargo clean
