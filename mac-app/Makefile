# Build the things-api menu bar .app bundle.
#
#   make            -> builds build/ThingsAPIMenuBar.app
#   make run        -> builds + launches the .app
#   make clean      -> remove build artifacts
#   make sign       -> code-sign the bundle (requires Developer ID cert)
#   make notarize   -> submit signed bundle for notarization
#
# CODESIGN_IDENTITY  e.g. "Developer ID Application: Your Name (TEAMID)"
# NOTARY_PROFILE     name of an `xcrun notarytool store-credentials` profile

APP_NAME       := ThingsAPIMenuBar
DISPLAY_NAME   := things-api
BUILD_DIR      := build
APP_BUNDLE     := $(BUILD_DIR)/$(APP_NAME).app
BUNDLE_MACOS   := $(APP_BUNDLE)/Contents/MacOS
BUNDLE_RES     := $(APP_BUNDLE)/Contents/Resources

# Swift build output
SWIFT_BUILD    := .build/release/$(APP_NAME)

# Rust binary (built from the workspace root, one level up)
RUST_DIR       := ..
RUST_BIN       := $(RUST_DIR)/target/release/things-api

CODESIGN_IDENTITY ?=

.PHONY: all swift rust app clean run sign notarize

all: app

# Compile the Swift menu bar binary.
swift:
	swift build -c release

# Compile the Rust things-api binary in the parent workspace.
rust:
	cd $(RUST_DIR) && cargo build --release -p things-api

# Assemble the .app bundle. Depends on both binaries.
app: swift rust
	@echo "→ Assembling $(APP_BUNDLE)"
	@rm -rf $(APP_BUNDLE)
	@mkdir -p $(BUNDLE_MACOS) $(BUNDLE_RES)
	@cp Info.plist $(APP_BUNDLE)/Contents/Info.plist
	@cp $(SWIFT_BUILD) $(BUNDLE_MACOS)/$(APP_NAME)
	@cp $(RUST_BIN) $(BUNDLE_MACOS)/things-api
	@chmod +x $(BUNDLE_MACOS)/$(APP_NAME) $(BUNDLE_MACOS)/things-api
	@echo "✓ Built $(APP_BUNDLE)"

run: app
	open $(APP_BUNDLE)

clean:
	rm -rf $(BUILD_DIR) .build
	cd $(RUST_DIR) && cargo clean -p things-api

# Sign the bundled binaries + the bundle itself with hardened runtime.
sign: app
ifeq ($(CODESIGN_IDENTITY),)
	$(error CODESIGN_IDENTITY is not set. Pass CODESIGN_IDENTITY='Developer ID Application: Name (TEAMID)')
endif
	@echo "→ Signing with: $(CODESIGN_IDENTITY)"
	codesign --force --options runtime --timestamp \
		--sign "$(CODESIGN_IDENTITY)" \
		$(BUNDLE_MACOS)/things-api
	codesign --force --options runtime --timestamp \
		--sign "$(CODESIGN_IDENTITY)" \
		$(BUNDLE_MACOS)/$(APP_NAME)
	codesign --force --options runtime --timestamp \
		--sign "$(CODESIGN_IDENTITY)" \
		$(APP_BUNDLE)
	@echo "✓ Signed"

notarize: sign
ifndef NOTARY_PROFILE
	$(error NOTARY_PROFILE is not set. Run `xcrun notarytool store-credentials` first.)
endif
	@echo "→ Zipping for notarization"
	ditto -c -k --keepParent $(APP_BUNDLE) $(BUILD_DIR)/$(APP_NAME).zip
	xcrun notarytool submit $(BUILD_DIR)/$(APP_NAME).zip \
		--keychain-profile "$(NOTARY_PROFILE)" \
		--wait
	xcrun stapler staple $(APP_BUNDLE)
	@echo "✓ Notarized and stapled"
