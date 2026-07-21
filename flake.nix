{
  description = "Dash Chat development flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";

    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";

    tauri-driver.url = "github:dash-chat/tauri-driver";

    tauri-plugin-holochain.url = "github:darksoil-studio/tauri-plugin-holochain/main-0.6";

    # nixpkgs revs pinned only for the chromedrivers matching the e2e Android
    # devices' WebView majors (physical phones on 149/150, the emulator image
    # on 124). The harness materializes packages.e2e-chromedrivers via
    # `nix build --out-link` and Appium picks the right one per device.
    nixpkgs-chromedriver-150.url = "github:nixos/nixpkgs/421eebfd0ec7bccd4abe826ce62d7e6e83129493";
    nixpkgs-chromedriver-149.url = "github:nixos/nixpkgs/d25a391ba507bc1cb32a8a732a2deb0d9dd16ad6";
    nixpkgs-chromedriver-124.url = "github:nixos/nixpkgs/fcc7d2be753560cdf34228a398f7a44202f09aaa";
  };

  nixConfig = {
    extra-substituters = [
      "https://dash-chat.cachix.org"
      "https://holochain-ci.cachix.org"
      "https://darksoil-studio.cachix.org"
    ];
    extra-trusted-public-keys = [
      "dash-chat.cachix.org-1:oAsoaEZ7e4UJlveRXF45MJ1P+Tf3OKFN5QkB8BuPaiM="
      "holochain-ci.cachix.org-1:5IUSkZc0aoRS53rfkvH9Kid40NpyjwCMCzwRTXy+QN8="
      "darksoil-studio.cachix.org-1:UEi+aujy44s41XL/pscLw37KEVpTEIn8N/kn7jO8rkc="
    ];
  };

  outputs =
    inputs:
    inputs.flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        ./nix/docker.nix
        ./nix/android-emulator.nix
        ./nix/e2e-chromedrivers.nix
        ./nix/tauri-app.nix
        ./crates/mailbox-server/default.nix
        ./crates/push-notifications-server/default.nix
      ];

      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];

      perSystem =
        {
          inputs',
          self',
          lib,
          system,
          ...
        }:
        let
          overlays = [ (import inputs.rust-overlay) ];
          pkgs = import inputs.nixpkgs { inherit system overlays; };

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
            pango
          ];
          nodeVersion = lib.versions.major (lib.strings.trim (builtins.readFile ./.node-version));
          # One toolchain (host + android targets) shared by the default and
          # androidDev shells: identical rustc + host-build env keep artifacts
          # in the shared target dir fingerprint-compatible across shells.
          rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          hostBuildEnvHook = lib.optionalString pkgs.stdenv.isLinux ''
            export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-args=-Wl,-rpath,${lib.makeLibraryPath tauriLibraries}"
            export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-args=-Wl,-rpath,${lib.makeLibraryPath tauriLibraries}"
            export SOURCE_DATE_EPOCH=315532800
          '';
          packages = [
            pkgs.mprocs
            pkgs.just
            pkgs."nodejs_${nodeVersion}"
            pkgs.pnpm
            pkgs.cargo-nextest
            pkgs.doctl
            inputs'.tauri-driver.packages.tauri-driver
          ];
        in
        rec {
          devShells.default = pkgs.mkShell {
            packages = [ rust ] ++ packages;
            inputsFrom = [ inputs'.tauri-plugin-holochain.devShells.holochainTauriDev ];
            shellHook = hostBuildEnvHook;
          };

          devShells.androidDev = pkgs.mkShell {
            packages = [
              rust
              pkgs."nodejs_${nodeVersion}"
              pkgs.jdk
            ]
            ++ lib.optionals (system == "x86_64-linux") [ self'.packages.boot-emulator ];
            inputsFrom = [ inputs'.tauri-plugin-holochain.devShells.androidDev ];
            # The e2e harness consumes tools and artifacts from PATH and
            # conventional paths; this hook provides the chromedrivers dir.
            shellHook = hostBuildEnvHook + ''
              mkdir -p "$(git rev-parse --show-toplevel)/e2e-tests/.appium"
              ln -sfn ${self'.packages.e2e-chromedrivers} "$(git rev-parse --show-toplevel)/e2e-tests/.appium/chromedrivers"
            '';
          };

          devShells.iosDev =
            pkgs.mkShell {
              inputsFrom = [ devShells.default ];
              packages = [ rust ] ++ lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconv ];
              shellHook = lib.optionalString pkgs.stdenv.isDarwin ''
                # Make libiconv findable by the linker even when xcodebuild
                # strips NIX_LDFLAGS from the environment.
                export LIBRARY_PATH="${lib.makeLibraryPath [ pkgs.libiconv ]}''${LIBRARY_PATH:+:$LIBRARY_PATH}"

                # Unset SDKROOT so xcrun can locate the iOS SDK from Xcode.
                unset SDKROOT
              '';
            };

        };
    };
}
