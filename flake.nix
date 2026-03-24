{
  description = "Dash Chat development flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";

    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";
    garnix-lib = {
      url = "github:garnix-io/garnix-lib";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    nixos-generators.url = "github:nix-community/nixos-generators";

    tauri-driver.url = "github:dash-chat/tauri-driver";

    tauri-plugin-holochain.url =
      "github:darksoil-studio/tauri-plugin-holochain/main-0.6";
  };

  nixConfig = {
    extra-substituters = [
      "https://cache.garnix.io"
      "https://holochain-ci.cachix.org"
      "https://darksoil-studio.cachix.org"
    ];
    extra-trusted-public-keys = [
      "cache.garnix.io:CTFPyKSLcx5RMJKfLo5EEPUObbA78b0YQ2DTCJXqr9g="
      "holochain-ci.cachix.org-1:5IUSkZc0aoRS53rfkvH9Kid40NpyjwCMCzwRTXy+QN8="
      "darksoil-studio.cachix.org-1:UEi+aujy44s41XL/pscLw37KEVpTEIn8N/kn7jO8rkc="
    ];
  };

  outputs = inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        ./nix/servers.nix
        ./nix/tauri-app.nix
        ./crates/mailbox-server/default.nix
      ];

      systems =
        [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin" ];

      perSystem = { inputs', self', lib, system, ... }:
        let
          overlays = [ (import inputs.rust-overlay) ];
          pkgs = import inputs.nixpkgs {
            inherit system overlays;
            config = {
              allowUnfreePredicate = pkg:
                let
                  name = lib.getName pkg;
                  androidUnfreeNames = [
                    "androidsdk"
                    "platform-tools"
                    "build-tools"
                    "cmdline-tools"
                    "tools"
                    "platforms"
                    "sources"
                    "system-images"
                    "emulator"
                    "ndk"
                    "cmake"
                    "patcher"
                  ];
                in lib.hasPrefix "android-sdk-" name
                || builtins.elem name androidUnfreeNames;
              android_sdk.accept_license = true;
            };
          };

          tauriLibraries = with pkgs; [
            webkitgtk_4_1
            gtk3
            cairo
            gdk-pixbuf
            glib
            dbus
            openssl
            librsvg
            libsoup_3
            libayatana-appindicator
          ];
          packages = [
            pkgs.mprocs
            pkgs.pnpm
            inputs'.tauri-driver.packages.tauri-driver
          ];
        in rec {

          devShells.default = let
            rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          in pkgs.mkShell {
            packages = [ rust ] ++ packages;
            inputsFrom =
              [ inputs'.tauri-plugin-holochain.devShells.holochainTauriDev ];
            shellHook = lib.optionalString pkgs.stdenv.isLinux ''
              export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-args=-Wl,-rpath,${
                lib.makeLibraryPath tauriLibraries
              }"
              export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-args=-Wl,-rpath,${
                lib.makeLibraryPath tauriLibraries
              }"
            '';
          };

          devShells.androidDev = let
            rust = pkgs.rust-bin.fromRustupToolchainFile
              ./rust-toolchain.android.toml;
            androidCompositionR26 = pkgs.androidenv.composeAndroidPackages {
              includeNDK = true;
              ndkVersion = "26.3.11579264";
            };
            androidSdkR26 = androidCompositionR26.androidsdk;
            # On Linux, C_INCLUDE_PATH from nix host packages leaks into Android
            # CC subprocesses, causing ring 'uintptr_t and size_t differ' errors.
            # We can't fix this via shellHook (nix re-applies derivation vars afterward),
            # so we wrap each NDK compiler to unset the host headers at invocation time.
            linuxNdkPrebuilt = "${androidSdkR26}/libexec/android-sdk/ndk-bundle/toolchains/llvm/prebuilt/linux-x86_64";
            mkAndroidCC = bin: pkgs.writeShellScript "${bin}-android-isolated" ''
              unset CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH OBJC_INCLUDE_PATH
              exec "${linuxNdkPrebuilt}/bin/${bin}" "$@"
            '';
          in pkgs.mkShell {
            packages = [ rust androidSdkR26 ] ++ packages;
            inputsFrom = [
              devShells.default
              inputs'.tauri-plugin-holochain.devShells.holochainTauriAndroidDev
            ];
            shellHook = ''
              # Pin NDK to r26 for Linux compatibility with ring during Android builds.
              export ANDROID_NDK_HOME="${androidSdkR26}/libexec/android-sdk/ndk-bundle"
              export ANDROID_NDK_LATEST_HOME="$ANDROID_NDK_HOME"
              export ANDROID_NDK="$ANDROID_NDK_HOME"
              export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"
              export NDK_HOME="$ANDROID_NDK_HOME"

              # Used by tauri plugin build scripts when targeting Android.
              export WRY_ANDROID_LIBRARY="''${WRY_ANDROID_LIBRARY:-tauri_app_lib}"

              for d in "$ANDROID_NDK_HOME"/toolchains/llvm/prebuilt/*; do
                export NDK_PREBUILT_DIR="$d"
                break
              done

              export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$NDK_PREBUILT_DIR/bin/aarch64-linux-android24-clang"

              export AR="$NDK_PREBUILT_DIR/bin/llvm-ar"
              export RANLIB="$NDK_PREBUILT_DIR/bin/llvm-ranlib"
              export CLANG_PATH="$NDK_PREBUILT_DIR/bin/clang"

              ${lib.optionalString pkgs.stdenv.isLinux ''
              # On Linux, use wrapper scripts to clear host C include paths at
              # compiler invocation time (shellHook cannot unset nix derivation vars).
              export CC_aarch64_linux_android="${mkAndroidCC "aarch64-linux-android24-clang"}"
              export CC_armv7_linux_androideabi="${mkAndroidCC "armv7a-linux-androideabi24-clang"}"
              export CC_i686_linux_android="${mkAndroidCC "i686-linux-android24-clang"}"
              export CC_x86_64_linux_android="${mkAndroidCC "x86_64-linux-android24-clang"}"
              export CXX_aarch64_linux_android="${mkAndroidCC "aarch64-linux-android24-clang++"}"
              export CXX_armv7_linux_androideabi="${mkAndroidCC "armv7a-linux-androideabi24-clang++"}"
              export CXX_i686_linux_android="${mkAndroidCC "i686-linux-android24-clang++"}"
              export CXX_x86_64_linux_android="${mkAndroidCC "x86_64-linux-android24-clang++"}"
              ''}
              ${lib.optionalString (!pkgs.stdenv.isLinux) ''
              export CC_aarch64_linux_android="$NDK_PREBUILT_DIR/bin/aarch64-linux-android24-clang"
              export CC_armv7_linux_androideabi="$NDK_PREBUILT_DIR/bin/armv7a-linux-androideabi24-clang"
              export CC_i686_linux_android="$NDK_PREBUILT_DIR/bin/i686-linux-android24-clang"
              export CC_x86_64_linux_android="$NDK_PREBUILT_DIR/bin/x86_64-linux-android24-clang"
              export CXX_aarch64_linux_android="$NDK_PREBUILT_DIR/bin/aarch64-linux-android24-clang++"
              export CXX_armv7_linux_androideabi="$NDK_PREBUILT_DIR/bin/armv7a-linux-androideabi24-clang++"
              export CXX_i686_linux_android="$NDK_PREBUILT_DIR/bin/i686-linux-android24-clang++"
              export CXX_x86_64_linux_android="$NDK_PREBUILT_DIR/bin/x86_64-linux-android24-clang++"
              ''}

              export CFLAGS_aarch64_linux_android="--target=aarch64-linux-android24 --sysroot=$NDK_PREBUILT_DIR/sysroot"
              export CFLAGS_armv7_linux_androideabi="--target=armv7-linux-androideabi24 --sysroot=$NDK_PREBUILT_DIR/sysroot"
              export CFLAGS_i686_linux_android="--target=i686-linux-android24 --sysroot=$NDK_PREBUILT_DIR/sysroot"
              export CFLAGS_x86_64_linux_android="--target=x86_64-linux-android24 --sysroot=$NDK_PREBUILT_DIR/sysroot"

              export CXXFLAGS_aarch64_linux_android="--target=aarch64-linux-android24"
              export CXXFLAGS_armv7_linux_androideabi="--target=armv7-linux-androideabi24"
              export CXXFLAGS_i686_linux_android="--target=i686-linux-android24"
              export CXXFLAGS_x86_64_linux_android="--target=x86_64-linux-android24"

              export BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android="--sysroot=$NDK_PREBUILT_DIR/sysroot -I$NDK_PREBUILT_DIR/sysroot/usr/include/aarch64-linux-android"
              export BINDGEN_EXTRA_CLANG_ARGS_armv7_linux_androideabi="--sysroot=$NDK_PREBUILT_DIR/sysroot -I$NDK_PREBUILT_DIR/sysroot/usr/include/arm-linux-androideabi"
              export BINDGEN_EXTRA_CLANG_ARGS_i686_linux_android="--sysroot=$NDK_PREBUILT_DIR/sysroot -I$NDK_PREBUILT_DIR/sysroot/usr/include/i686-linux-android"
              export BINDGEN_EXTRA_CLANG_ARGS_x86_64_linux_android="--sysroot=$NDK_PREBUILT_DIR/sysroot -I$NDK_PREBUILT_DIR/sysroot/usr/include/x86_64-linux-android"
            '';
          };

          devShells.iosDev = let
            rust =
              pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.ios.toml;
          in pkgs.mkShell {
            inputsFrom = [ devShells.default ];
            packages = [ rust ]
              ++ lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconv ];
            shellHook = lib.optionalString pkgs.stdenv.isDarwin ''
              # Make libiconv findable by the linker even when xcodebuild
              # strips NIX_LDFLAGS from the environment.
              export LIBRARY_PATH="${
                lib.makeLibraryPath [ pkgs.libiconv ]
              }''${LIBRARY_PATH:+:$LIBRARY_PATH}"

              # Unset SDKROOT so xcrun can locate the iOS SDK from Xcode.
              unset SDKROOT
            '';
          };

        };
    };
}
