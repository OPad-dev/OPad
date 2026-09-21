# OPad top-level Makefile
# Implements packaging and build-from-source requirements (B-1..B-4, L-1).

SHELL := /bin/bash
.SHELLFLAGS := -euo pipefail -c

# System install paths (GNU autotools style, B-1)
PREFIX ?= /usr/local
DESTDIR ?=
BINDIR ?= $(PREFIX)/bin
DATADIR ?= $(PREFIX)/share
LIBDIR ?= $(PREFIX)/lib
SYSTEMDUSERDIR ?= $(LIBDIR)/systemd/user
UDEVRULESDIR ?= /usr/lib/udev/rules.d
APPLICATIONSDIR ?= $(DATADIR)/applications
INSTALL_ORIGIN ?= source

# User install paths (~/.local layout, B-2)
PREFIX_USER ?= $(HOME)/.local
BINDIR_USER ?= $(PREFIX_USER)/bin
DATADIR_USER ?= $(PREFIX_USER)/share
APPLICATIONSDIR_USER ?= $(DATADIR_USER)/applications
SYSTEMDUSERDIR_USER ?= $(HOME)/.config/systemd/user
AUTOSTARTDIR_USER ?= $(HOME)/.config/autostart
LIBDIR_USER ?= $(PREFIX_USER)/lib
INSTALL_ORIGIN_USER ?= user

# Build tools & flags
CARGO ?= cargo
CARGO_FLAGS ?= --release
NODE ?= node
PNPM ?= pnpm

# tosu source build configuration (B-4)
TOSU_REPO ?= https://github.com/KotRikD/tosu.git
TOSU_VERSION ?= v4.26.2
TOSU_SRC_DIR ?= build/tosu-src
TOSU_BUILD_DIR ?= build/tosu

TARGET_DIR ?= desktop/target/release
BINS := opad-daemon opad-gui opadctl

.PHONY: all tosu firmware install install-user uninstall uninstall-user check clean appimage deb rpm packages

# L-4: Build AppDir / AppImage
appimage: all
	packaging/linux/appimage/build_appimage.sh

# L-3: Build Debian package (.deb)
deb: all tosu
	cd desktop && cargo deb -p opad-gui --no-build -o ../dist/

# L-3: Build RPM package (.rpm)
rpm: all tosu
	cd desktop/gui && cargo generate-rpm --auto-req disabled -o ../../dist/

# Build all packages
packages:
	./scripts/release/build_packages.sh

# B-2: Default target - release-build desktop binaries
all:
	$(CARGO) build $(CARGO_FLAGS) --manifest-path desktop/Cargo.toml --bins

# B-4: Build tosu from source into build/tosu/
tosu:
	@mkdir -p $(TOSU_BUILD_DIR)
	@if [ ! -d "$(TOSU_SRC_DIR)" ]; then \
		echo "Cloning tosu source ($(TOSU_VERSION)) from $(TOSU_REPO)..."; \
		git clone --depth 1 --branch $(TOSU_VERSION) $(TOSU_REPO) $(TOSU_SRC_DIR); \
	fi
	@echo "Building tosu from source..."
	cd $(TOSU_SRC_DIR) && $(PNPM) install --frozen-lockfile
	cd $(TOSU_SRC_DIR) && $(PNPM) --filter tosu run genver && $(PNPM) --filter tosu run ts:compile
	mkdir -p $(TOSU_BUILD_DIR)/dist
	cp -r $(TOSU_SRC_DIR)/packages/tosu/dist/* $(TOSU_BUILD_DIR)/dist/
	@printf '#!/bin/sh\nDIR="$$(cd "$$(dirname "$$0")" && pwd)"\nif [ -f "$$DIR/index.js" ]; then\n  exec node "$$DIR/index.js" "$$@"\nelif [ -f "$$DIR/dist/index.js" ]; then\n  exec node "$$DIR/dist/index.js" "$$@"\nelse\n  echo "tosu: index.js not found in $$DIR" >&2\n  exit 1\nfi\n' > $(TOSU_BUILD_DIR)/tosu
	@chmod +x $(TOSU_BUILD_DIR)/tosu
	install -m 644 licenses/tosu/VERSION $(TOSU_BUILD_DIR)/VERSION
	install -m 644 licenses/tosu/NOTICE $(TOSU_BUILD_DIR)/NOTICE
	install -m 644 licenses/tosu/LICENSE $(TOSU_BUILD_DIR)/LICENSE
	@echo "✓ tosu built successfully in $(TOSU_BUILD_DIR)"

# B-2: Build firmware with ESP-IDF, cleanly skipped if absent
firmware:
	@if command -v idf.py >/dev/null 2>&1; then \
		echo "Building firmware with ESP-IDF..."; \
		idf.py -C firmware build; \
	else \
		echo "ESP-IDF (idf.py) not found in PATH; skipping firmware build."; \
	fi

# B-1, B-2, L-1: Install into $(DESTDIR)$(PREFIX) using system layout
install: all
	install -d "$(DESTDIR)$(BINDIR)"
	for bin in $(BINS); do \
		install -m 755 "$(TARGET_DIR)/$$bin" "$(DESTDIR)$(BINDIR)/$$bin"; \
	done
	install -d "$(DESTDIR)$(SYSTEMDUSERDIR)"
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/linux/systemd-user/opad-daemon.service.in > "$(DESTDIR)$(SYSTEMDUSERDIR)/opad-daemon.service"
	chmod 644 "$(DESTDIR)$(SYSTEMDUSERDIR)/opad-daemon.service"
	install -d "$(DESTDIR)$(APPLICATIONSDIR)"
	sed 's|@BINDIR@|$(BINDIR)|g' packaging/linux/opad.desktop.in > "$(DESTDIR)$(APPLICATIONSDIR)/opad.desktop"
	chmod 644 "$(DESTDIR)$(APPLICATIONSDIR)/opad.desktop"
	install -d "$(DESTDIR)$(UDEVRULESDIR)"
	install -m 644 packaging/linux/udev/70-opad.rules "$(DESTDIR)$(UDEVRULESDIR)/70-opad.rules"
	install -d "$(DESTDIR)$(LIBDIR)/opad"
	printf '%s\n' "$(INSTALL_ORIGIN)" > "$(DESTDIR)$(LIBDIR)/opad/install-origin"
	chmod 644 "$(DESTDIR)$(LIBDIR)/opad/install-origin"
	@if [ -d "packaging/linux/icons" ]; then \
		install -d "$(DESTDIR)$(DATADIR)/icons"; \
		cp -r packaging/linux/icons/* "$(DESTDIR)$(DATADIR)/icons/"; \
	fi
	@if [ -d "$(TOSU_BUILD_DIR)" ]; then \
		echo "Installing bundled tosu to $(DESTDIR)$(LIBDIR)/opad/tosu..."; \
		install -d "$(DESTDIR)$(LIBDIR)/opad/tosu"; \
		cp -r $(TOSU_BUILD_DIR)/dist/* "$(DESTDIR)$(LIBDIR)/opad/tosu/"; \
		install -m 755 "$(TOSU_BUILD_DIR)/tosu" "$(DESTDIR)$(LIBDIR)/opad/tosu/tosu"; \
		install -m 644 licenses/tosu/VERSION "$(DESTDIR)$(LIBDIR)/opad/tosu/VERSION"; \
		install -m 644 licenses/tosu/NOTICE "$(DESTDIR)$(LIBDIR)/opad/tosu/NOTICE"; \
		install -m 644 licenses/tosu/LICENSE "$(DESTDIR)$(LIBDIR)/opad/tosu/LICENSE"; \
	fi

# B-2: Install into ~/.local layout (replaces install.sh)
install-user: all
	install -d "$(BINDIR_USER)"
	for bin in $(BINS); do \
		install -m 755 "$(TARGET_DIR)/$$bin" "$(BINDIR_USER)/$$bin"; \
	done
	install -d "$(SYSTEMDUSERDIR_USER)"
	sed 's|@BINDIR@|$(BINDIR_USER)|g' packaging/linux/systemd-user/opad-daemon.service.in > "$(SYSTEMDUSERDIR_USER)/opad-daemon.service"
	chmod 644 "$(SYSTEMDUSERDIR_USER)/opad-daemon.service"
	install -d "$(APPLICATIONSDIR_USER)"
	sed 's|@BINDIR@|$(BINDIR_USER)|g' packaging/linux/opad.desktop.in > "$(APPLICATIONSDIR_USER)/opad.desktop"
	chmod 644 "$(APPLICATIONSDIR_USER)/opad.desktop"
	install -d "$(AUTOSTARTDIR_USER)"
	sed 's|@BINDIR@|$(BINDIR_USER)|g' packaging/linux/xdg-autostart/opad-gui.desktop.in > "$(AUTOSTARTDIR_USER)/opad-gui.desktop"
	chmod 644 "$(AUTOSTARTDIR_USER)/opad-gui.desktop"
	install -d "$(LIBDIR_USER)/opad"
	printf '%s\n' "$(INSTALL_ORIGIN_USER)" > "$(LIBDIR_USER)/opad/install-origin"
	chmod 644 "$(LIBDIR_USER)/opad/install-origin"
	@if [ -d "$(TOSU_BUILD_DIR)" ]; then \
		echo "Installing bundled tosu to $(LIBDIR_USER)/opad/tosu..."; \
		install -d "$(LIBDIR_USER)/opad/tosu"; \
		cp -r $(TOSU_BUILD_DIR)/dist/* "$(LIBDIR_USER)/opad/tosu/"; \
		install -m 755 "$(TOSU_BUILD_DIR)/tosu" "$(LIBDIR_USER)/opad/tosu/tosu"; \
		install -m 644 licenses/tosu/VERSION "$(LIBDIR_USER)/opad/tosu/VERSION"; \
		install -m 644 licenses/tosu/NOTICE "$(LIBDIR_USER)/opad/tosu/NOTICE"; \
		install -m 644 licenses/tosu/LICENSE "$(LIBDIR_USER)/opad/tosu/LICENSE"; \
	fi
	@echo ""
	@echo "=== User installation finished ==="
	@echo "Binaries installed to: $(BINDIR_USER)"
	@echo "To enable and start the daemon:"
	@echo "  systemctl --user daemon-reload"
	@echo "  systemctl --user enable --now opad-daemon.service"
	@echo "To grant non-root access to the pad (if not already done):"
	@echo "  sudo cp packaging/linux/udev/70-opad.rules /etc/udev/rules.d/"
	@echo "  sudo udevadm control --reload-rules && sudo udevadm trigger"

# B-2: Remove everything placed by install
uninstall:
	for bin in $(BINS); do \
		rm -f "$(DESTDIR)$(BINDIR)/$$bin"; \
	done
	rm -f "$(DESTDIR)$(SYSTEMDUSERDIR)/opad-daemon.service"
	rm -f "$(DESTDIR)$(APPLICATIONSDIR)/opad.desktop"
	rm -f "$(DESTDIR)$(UDEVRULESDIR)/70-opad.rules"
	rm -rf "$(DESTDIR)$(LIBDIR)/opad"
	@echo "✓ Uninstall complete."

uninstall-user:
	for bin in $(BINS); do \
		rm -f "$(BINDIR_USER)/$$bin"; \
	done
	rm -f "$(SYSTEMDUSERDIR_USER)/opad-daemon.service"
	rm -f "$(APPLICATIONSDIR_USER)/opad.desktop"
	rm -f "$(AUTOSTARTDIR_USER)/opad-gui.desktop"
	rm -rf "$(LIBDIR_USER)/opad"
	@echo "✓ User uninstall complete."

# B-2: Verification target
check:
	$(CARGO) fmt --manifest-path desktop/Cargo.toml --all -- --check
	$(CARGO) clippy --manifest-path desktop/Cargo.toml --all-targets -- -D warnings
	$(CARGO) test --manifest-path desktop/Cargo.toml
	./firmware/test/host/run_tests.sh
	@echo "✓ All checks passed!"

# B-2: Clean build artifacts
clean:
	$(CARGO) clean --manifest-path desktop/Cargo.toml
	rm -rf build/
