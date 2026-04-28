# lorawanwiz build recipes
#
# Copyright L. Weinzierl, 2026
#
# Project layout note: there used to be LICENSE-MIT and LICENSE-APACHE
# files in this directory from earlier iterations. Delete them by hand
# if they are still present; this project is not open source.

# default recipe (hidden) lists all available recipes
@_default:
    just --list

# run unit tests for the math module
test:
    cargo test --lib

# run the native (windowed) version
native:
    cargo run --release

# build the WASM bundle and stage it in dist/.
#
# If the build fails with a "wasm-bindgen schema version mismatch" error
# from wasm-bindgen-cli, run the exact `cargo update` command that error
# message prints, then `just wasm` again. The CLI and the Rust crate must
# share the same schema version. Rare and self-resolving.
wasm:
    @command -v wasm-bindgen >/dev/null 2>&1 || { \
        echo "wasm-bindgen-cli not installed."; \
        echo "Install with: cargo install -f wasm-bindgen-cli"; \
        exit 1; \
    }
    rustup target add wasm32-unknown-unknown
    cargo build --target wasm32-unknown-unknown --profile wasm-release
    mkdir -p dist
    wasm-bindgen \
        --target web \
        --out-dir dist \
        --out-name lorawanwiz \
        --no-typescript \
        target/wasm32-unknown-unknown/wasm-release/lorawanwiz.wasm
    cp index.html dist/
    cp assets/favicon.svg dist/
    cp assets/favicon.ico dist/ 2>/dev/null || true
    cp assets/favicon-96x96.png dist/ 2>/dev/null || true
    cp assets/apple-touch-icon.png dist/ 2>/dev/null || true
    cp assets/site.webmanifest dist/ 2>/dev/null || true
    cp assets/web-app-manifest-192x192.png dist/ 2>/dev/null || true
    cp assets/web-app-manifest-512x512.png dist/ 2>/dev/null || true
    @echo
    @echo "Bundle ready in dist/. Serve with: just serve"
    @echo "(wasm-opt is intentionally not run; it triggers a 'failed to grow"
    @echo "table' error at runtime in some Bevy 0.18 + wasm-bindgen combos."
    @echo "If you want to optimize size manually, run:"
    @echo "  wasm-opt -O2 --enable-bulk-memory --enable-reference-types \\"
    @echo "    -o dist/lorawanwiz_bg.opt.wasm dist/lorawanwiz_bg.wasm"
    @echo "  mv dist/lorawanwiz_bg.opt.wasm dist/lorawanwiz_bg.wasm)"

# build the WASM bundle (if needed) and serve it on http://localhost:8080
serve: wasm
    cd dist && python3 -m http.server 8080

# build the full Zola site with the embedded WASM app at /app/. Used by
# the GitHub Pages deploy workflow. Hidden from `just --list`.
@_site BASE_URL='/':
    just wasm
    mkdir -p website/static/app website/static
    cp -r dist/* website/static/app/
    cp assets/favicon.svg website/static/
    cp assets/favicon.ico website/static/ 2>/dev/null || true
    cp assets/favicon-96x96.png website/static/ 2>/dev/null || true
    cp assets/apple-touch-icon.png website/static/ 2>/dev/null || true
    cp assets/site.webmanifest website/static/ 2>/dev/null || true
    cp assets/web-app-manifest-192x192.png website/static/ 2>/dev/null || true
    cp assets/web-app-manifest-512x512.png website/static/ 2>/dev/null || true
    cd website && zola build --base-url '{{ BASE_URL }}'
    @echo "Site built into website/public/"

# install the desktop entry and icons for the current user (Linux).
# Builds the release binary first if needed, then writes a .desktop file
# with absolute paths so it works regardless of $PATH.
install-linux:
    #!/usr/bin/env bash
    set -euo pipefail

    if [ ! -x target/release/lorawanwiz ]; then
        echo "Building release binary first..."
        cargo build --release
    fi

    BIN_PATH="$(pwd)/target/release/lorawanwiz"
    ICON_DIR="$HOME/.local/share/icons/hicolor"
    ICON_PATH="$ICON_DIR/scalable/apps/lorawanwiz.svg"
    APPS_DIR="$HOME/.local/share/applications"
    DESKTOP_FILE="$APPS_DIR/lorawanwiz.desktop"

    mkdir -p \
        "$APPS_DIR" \
        "$ICON_DIR/scalable/apps" \
        "$ICON_DIR/192x192/apps" \
        "$ICON_DIR/512x512/apps"

    cp assets/favicon.svg "$ICON_PATH"
    if [ -f assets/web-app-manifest-192x192.png ]; then
        cp assets/web-app-manifest-192x192.png "$ICON_DIR/192x192/apps/lorawanwiz.png"
    fi
    if [ -f assets/web-app-manifest-512x512.png ]; then
        cp assets/web-app-manifest-512x512.png "$ICON_DIR/512x512/apps/lorawanwiz.png"
    fi

    # Write the .desktop file with absolute paths directly. No sed: write
    # the literal contents we want, baking in the resolved $BIN_PATH and
    # $ICON_PATH. This avoids any escaping issues.
    cat > "$DESKTOP_FILE" <<EOF
    [Desktop Entry]
    Type=Application
    Name=lorawanwiz
    GenericName=LoRaWAN modulation visualizer
    Comment=Step-by-step LoRaWAN encryption and chirp modulation, for technical talks
    Exec=$BIN_PATH
    Icon=$ICON_PATH
    Terminal=false
    Categories=Education;
    Keywords=LoRaWAN;LoRa;modulation;chirp;visualizer;
    X-Copyright=Copyright L. Weinzierl, 2026
    X-Homepage=https://weinzierlweb.com/
    EOF

    chmod +x "$DESKTOP_FILE"

    if command -v desktop-file-validate >/dev/null 2>&1; then
        desktop-file-validate "$DESKTOP_FILE" || echo "(validation warnings, install continues)"
    fi
    if command -v gio >/dev/null 2>&1; then
        gio set "$DESKTOP_FILE" metadata::trusted true 2>/dev/null || true
    fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then
        gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
    fi
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "$APPS_DIR" 2>/dev/null || true
    fi
    if command -v xdg-desktop-menu >/dev/null 2>&1; then
        xdg-desktop-menu forceupdate 2>/dev/null || true
    fi

    echo
    echo "Installed lorawanwiz.desktop with absolute paths:"
    echo "  Exec=$BIN_PATH"
    echo "  Icon=$ICON_PATH"
    echo
    echo "Run 'just check-icon' to diagnose if the icon does not appear."
    echo "If everything looks correct there, log out and back in."

# print diagnostic information about the installed icon and .desktop file.
check-icon:
    #!/usr/bin/env bash
    set +e

    DESKTOP_FILE="$HOME/.local/share/applications/lorawanwiz.desktop"
    ICON_DIR="$HOME/.local/share/icons"

    echo "=== .desktop file ==="
    if [ -f "$DESKTOP_FILE" ]; then
        ls -la "$DESKTOP_FILE"
    else
        echo "MISSING: $DESKTOP_FILE"
    fi
    echo

    echo "=== .desktop file content ==="
    if [ -f "$DESKTOP_FILE" ]; then
        cat "$DESKTOP_FILE"
    else
        echo "(not installed)"
    fi
    echo

    echo "=== Icon files installed ==="
    if find "$ICON_DIR" -name 'lorawanwiz*' -ls 2>/dev/null | grep -q .; then
        find "$ICON_DIR" -name 'lorawanwiz*' -ls
    else
        echo "(none found)"
    fi
    echo

    echo "=== Validation ==="
    if command -v desktop-file-validate >/dev/null 2>&1; then
        desktop-file-validate "$DESKTOP_FILE" 2>&1 \
            && echo ".desktop file is valid"
    else
        echo "desktop-file-validate not installed"
    fi
    echo

    echo "=== Trusted flag (GNOME) ==="
    if command -v gio >/dev/null 2>&1; then
        gio info "$DESKTOP_FILE" 2>/dev/null | grep -i trusted \
            || echo "(no trusted flag set)"
    else
        echo "(gio not available)"
    fi
    echo

    echo "=== Binary exists ==="
    if [ -x target/release/lorawanwiz ]; then
        ls -la target/release/lorawanwiz
    else
        echo "MISSING: target/release/lorawanwiz (run 'cargo build --release')"
    fi
    echo

    echo "=== Exec target accessible ==="
    EXEC_LINE=$(grep '^Exec=' "$DESKTOP_FILE" 2>/dev/null | sed 's/^Exec=//')
    if [ -n "$EXEC_LINE" ]; then
        if [ -x "$EXEC_LINE" ]; then
            echo "OK: $EXEC_LINE is executable"
        else
            echo "BROKEN: $EXEC_LINE does not exist or is not executable"
        fi
    fi
    echo

    echo "=== Icon path accessible ==="
    ICON_LINE=$(grep '^Icon=' "$DESKTOP_FILE" 2>/dev/null | sed 's/^Icon=//')
    if [ -n "$ICON_LINE" ]; then
        if [ -f "$ICON_LINE" ]; then
            echo "OK: $ICON_LINE exists"
        else
            echo "BROKEN: $ICON_LINE does not exist"
        fi
    fi
    echo

    echo "If everything above is OK and the icon still does not show,"
    echo "log out and back in. GNOME caches launcher entries aggressively."

# clean build artifacts
clean:
    cargo clean
    rm -rf dist website/public website/static/app
