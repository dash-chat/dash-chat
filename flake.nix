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
              export CARGO_BUILD_RUSTFLAGS="-C link-args=-Wl,-rpath,${
                lib.makeLibraryPath tauriLibraries
              }"
            '';
          };

          devShells.androidDev = let
            rust = pkgs.rust-bin.fromRustupToolchainFile
              ./rust-toolchain.android.toml;
          in pkgs.mkShell {
            packages = [ rust ];
            inputsFrom = [
              devShells.default
              inputs'.tauri-plugin-holochain.devShells.holochainTauriAndroidDev
            ];
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
