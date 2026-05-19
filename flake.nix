{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          overlays = [
            (import rust-overlay)
          ];
          pkgs = import nixpkgs {
            inherit system overlays;
            config = {
              android_sdk.accept_license = true;
              allowUnfree = true;
            };
          };
          gst = pkgs.gst_all_1;
          lib = pkgs.lib;

          # ── Rust toolchain with Android cross-compilation targets ────
          rustToolchain = pkgs.rust-bin.selectLatestNightlyWith (toolchain:
            toolchain.default.override {
              targets = [
                "aarch64-linux-android"
                "armv7-linux-androideabi"
                "x86_64-linux-android"
                "i686-linux-android"
              ];
            }
          );
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };

          # ── Android SDK + NDK (r25c = 25.2.9519653) ────────────────
          # NDK r25c matches CI and build.rs clang/14.0.7 path.
          ndkVersion = "25.2.9519653";
          androidComposition = pkgs.androidenv.composeAndroidPackages {
            # 34.0.0 matches AGP 8.7's auto-resolved build-tools for compileSdk 34
            # (CI silently auto-installs it; the Nix SDK is read-only so we pin it here).
            # 35.0.0 mirrors what ci/.github/actions/android-ci-setup installs.
            buildToolsVersions = [ "34.0.0" "35.0.0" ];
            platformVersions = [ "34" "35" ];
            ndkVersions = [ ndkVersion ];
            includeNDK = true;
            includeSources = false;
            includeSystemImages = false;
            abiVersions = [ "arm64-v8a" "armeabi-v7a" "x86_64" "x86" ];
          };
          androidSdk = androidComposition.androidsdk;
          androidHome = "${androidSdk}/libexec/android-sdk";
          androidNdkRoot = "${androidHome}/ndk/${ndkVersion}";

          # ── GStreamer Android version (matches CI workflow) ─────────
          gstVersion = "1.28.0";

          commonPkgConfigDeps = [
            gst.gstreamer
            gst.gst-plugins-base
            gst.gst-plugins-good
            gst.gst-plugins-bad
            gst.gst-plugins-ugly
            gst.gst-libav
            pkgs.openssl
            pkgs.glib
            pkgs.pango
            pkgs.cairo
            pkgs.fontconfig
            pkgs.freetype
          ];
          linuxGuiDeps = lib.optionals pkgs.stdenv.hostPlatform.isLinux [
            pkgs.alsa-lib
            pkgs.libpulseaudio
            pkgs.libnice
            pkgs.pipewire
            pkgs.libxkbcommon
            pkgs.wayland
            pkgs.libGL
            pkgs.vulkan-loader
            pkgs.libx11
            pkgs.libxcursor
            pkgs.libxi
            pkgs.libxrandr
            pkgs.libxcb
            pkgs.libxext
          ];
        in
        {
          devShells.default = pkgs.mkShell {
            # Fast shell for local checks/tests that do not need Android SDK/NDK.
            nativeBuildInputs = with pkgs; [
              rustToolchain
              pkg-config
              libclang
              clang
              gnumake
            ];

            buildInputs = with pkgs; [
              openssl
              glib
              glib-networking
              pango
              cairo
              fontconfig
              freetype
              gst.gstreamer
              gst.gst-plugins-base
              gst.gst-plugins-good
              gst.gst-plugins-bad
              gst.gst-plugins-ugly
              gst.gst-libav
            ] ++ linuxGuiDeps;

            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
            PKG_CONFIG_ALLOW_CROSS = 1;
            PKG_CONFIG_PATH = pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" (
              commonPkgConfigDeps ++ linuxGuiDeps
            );

            shellHook = ''
              export GIO_EXTRA_MODULES="${pkgs.glib-networking}/lib/gio/modules"
              export RUST_BACKTRACE=1
              echo "fcast dev shell ready"
              echo "Run tests with: cargo test -p android-sender screen_capture"
            '';
          };

          devShells.android = pkgs.mkShell {
            nativeBuildInputs = with pkgs; [
              rustToolchain
              cargo-ndk
              pkg-config
              libclang
              clang
              android-tools      # adb, fastboot
              androidSdk         # full Android SDK + NDK
              wget
              unzip
              gnutar
              xz                 # for .tar.xz extraction
              gnumake
              jdk21_headless
            ];

            buildInputs = with pkgs; [
              openssl
              glib
              glib-networking
              pango
              cairo
              fontconfig
              freetype
              gst.gstreamer
              gst.gst-plugins-base
              gst.gst-plugins-good
              gst.gst-plugins-bad
              gst.gst-plugins-ugly
              gst.gst-libav
            ] ++ linuxGuiDeps;

            LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";
            PKG_CONFIG_ALLOW_CROSS = 1;
            PKG_CONFIG_PATH = pkgs.lib.makeSearchPathOutput "dev" "lib/pkgconfig" (
              commonPkgConfigDeps ++ linuxGuiDeps
            );

            # ── Android environment variables ─────────────────────────
            ANDROID_HOME = androidHome;
            ANDROID_SDK_ROOT = androidHome;
            ANDROID_NDK_ROOT = androidNdkRoot;
            ANDROID_NDK_HOME = androidNdkRoot;
            ANDROID_NDK = androidNdkRoot;

            shellHook = ''
              export GIO_EXTRA_MODULES="${pkgs.glib-networking}/lib/gio/modules"
              export RUST_BACKTRACE=1
              export ANDROID_NDK="$ANDROID_NDK_ROOT"

              NDK_PREBUILT="$ANDROID_NDK_ROOT/toolchains/llvm/prebuilt/darwin-x86_64/bin"
              export CC_aarch64_linux_android="$NDK_PREBUILT/aarch64-linux-android26-clang"
              export CXX_aarch64_linux_android="$NDK_PREBUILT/aarch64-linux-android26-clang++"
              export AR_aarch64_linux_android="$NDK_PREBUILT/llvm-ar"
              export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$CC_aarch64_linux_android"

              export CC_armv7_linux_androideabi="$NDK_PREBUILT/armv7a-linux-androideabi26-clang"
              export CXX_armv7_linux_androideabi="$NDK_PREBUILT/armv7a-linux-androideabi26-clang++"
              export AR_armv7_linux_androideabi="$NDK_PREBUILT/llvm-ar"
              export CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER="$CC_armv7_linux_androideabi"

              export CC_x86_64_linux_android="$NDK_PREBUILT/x86_64-linux-android26-clang"
              export CXX_x86_64_linux_android="$NDK_PREBUILT/x86_64-linux-android26-clang++"
              export AR_x86_64_linux_android="$NDK_PREBUILT/llvm-ar"
              export CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$CC_x86_64_linux_android"

              export CC_i686_linux_android="$NDK_PREBUILT/i686-linux-android26-clang"
              export CXX_i686_linux_android="$NDK_PREBUILT/i686-linux-android26-clang++"
              export AR_i686_linux_android="$NDK_PREBUILT/llvm-ar"
              export CARGO_TARGET_I686_LINUX_ANDROID_LINKER="$CC_i686_linux_android"

              # ── Generate local.properties for Gradle ────────────────
              # Always regenerate so Gradle picks up the current Nix SDK path
              # after flake bumps (otherwise a stale sdk.dir hash silently
              # routes Gradle back to a derivation that no longer has the
              # expected build-tools/platforms).
              EXPECTED_SDK_DIR="sdk.dir=${androidHome}"
              if [ ! -f local.properties ] || ! grep -qxF "$EXPECTED_SDK_DIR" local.properties; then
                echo "$EXPECTED_SDK_DIR" > local.properties
                echo "ndk.dir=${androidNdkRoot}" >> local.properties
                echo "✓ Generated local.properties"
              fi

              # ── Download GStreamer Android prebuilt binaries ─────────
              GST_ANDROID_DIR="$PWD/.android/gstreamer"
              if [ ! -d "$GST_ANDROID_DIR/arm64" ]; then
                echo ""
                echo "┌──────────────────────────────────────────────────────┐"
                echo "│ Downloading GStreamer ${gstVersion} Android binaries...    │"
                echo "│ (~600 MB, one-time download)                        │"
                echo "└──────────────────────────────────────────────────────┘"
                echo ""
                mkdir -p "$GST_ANDROID_DIR"
                GST_TAR="gstreamer-1.0-android-universal-${gstVersion}.tar.xz"
                GST_URL="https://gstreamer.freedesktop.org/pkg/android/${gstVersion}/$GST_TAR"
                if wget -q --show-progress -O "/tmp/$GST_TAR" "$GST_URL"; then
                  tar xf "/tmp/$GST_TAR" -C "$GST_ANDROID_DIR"
                  rm -f "/tmp/$GST_TAR"
                  echo "✓ GStreamer Android binaries installed"
                else
                  echo "✗ Failed to download GStreamer Android."
                  echo "  Manual download: $GST_URL"
                  echo "  Extract to: $GST_ANDROID_DIR"
                fi
              fi
              export GSTREAMER_ROOT_ANDROID="$GST_ANDROID_DIR"
              export PKG_CONFIG_PATH="$GSTREAMER_ROOT_ANDROID/arm64/lib/pkgconfig"

              echo ""
              echo "┌──────────────────────────────────────────────────────┐"
              echo "│  fcast-android-sender dev shell                     │"
              echo "├──────────────────────────────────────────────────────┤"
              echo "│  ANDROID_HOME : $ANDROID_HOME"
              echo "│  NDK          : $ANDROID_NDK_ROOT"
              echo "│  GStreamer    : $GSTREAMER_ROOT_ANDROID"
              echo "│                                                      │"
              echo "│  Quick start:                                        │"
              echo "│    ./scripts/build-deploy.sh          (build+install) │"
              echo "│    ./scripts/build-deploy.sh --release (release APK) │"
              echo "│    adb logcat -s fcastsender          (view logs)    │"
              echo "└──────────────────────────────────────────────────────┘"
            '';
          };
        }
      );
}
