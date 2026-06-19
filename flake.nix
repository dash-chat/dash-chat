{
  description = "Dash Chat development flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";

    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";

    tauri-driver.url = "github:dash-chat/tauri-driver";

    tauri-plugin-holochain.url = "github:darksoil-studio/tauri-plugin-holochain/main-0.6";
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
            # Microphone capture for voice notes (cpal → ALSA) on Linux desktop.
            alsa-lib
            # GStreamer so WebKitGTK can play voice-note audio (<audio> WAV).
            gst_all_1.gstreamer
            gst_all_1.gst-plugins-base
            gst_all_1.gst-plugins-good
          ];
          gstPluginPath = pkgs.lib.makeSearchPathOutput "lib" "lib/gstreamer-1.0" [
            pkgs.gst_all_1.gstreamer
            pkgs.gst_all_1.gst-plugins-base
            pkgs.gst_all_1.gst-plugins-good
          ];
          # ALSA config that routes the `default` device through PulseAudio
          # (PipeWire-pulse) so cpal mic capture works from the dev shell. The
          # nixpkgs alsa-lib we link/rpath otherwise resolves `default` to a
          # nonexistent hw device.
          alsaConf = pkgs.writeText "asound.conf" ''
            <${pkgs.alsa-lib}/share/alsa/alsa.conf>
            <${pkgs.alsa-plugins}/share/alsa/alsa.conf.d/50-pulseaudio.conf>
            pcm.!default { type pulse }
            ctl.!default { type pulse }
          '';
          nodeVersion = lib.versions.major (lib.strings.trim (builtins.readFile ./.node-version));
          packages = [
            pkgs.mprocs
            pkgs.just
            pkgs."nodejs_${nodeVersion}"
            pkgs.pnpm
            pkgs.cargo-nextest
            pkgs.doctl
            inputs'.tauri-driver.packages.tauri-driver
            # Build-time discovery of ALSA for cpal (voice-note recording).
            pkgs.pkg-config
            pkgs.alsa-lib
          ];
        in
        rec {
          devShells.default =
            let
              rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
            in
            pkgs.mkShell {
              packages = [ rust ] ++ packages;
              inputsFrom = [ inputs'.tauri-plugin-holochain.devShells.holochainTauriDev ];
              shellHook = lib.optionalString pkgs.stdenv.isLinux ''
                export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-args=-Wl,-rpath,${lib.makeLibraryPath tauriLibraries}"
                export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-args=-Wl,-rpath,${lib.makeLibraryPath tauriLibraries}"
                # Let WebKitGTK find GStreamer plugins for <audio> playback.
                export GST_PLUGIN_SYSTEM_PATH_1_0="${gstPluginPath}"
                # Route ALSA (cpal mic capture) to the system PulseAudio/PipeWire
                # so the `default` capture device resolves in the dev shell.
                export ALSA_PLUGIN_DIR="${pkgs.alsa-plugins}/lib/alsa-lib"
                export ALSA_CONFIG_PATH="${alsaConf}"
              '';
            };

          devShells.androidDev =
            let
              rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.android.toml;
            in
            pkgs.mkShell {
              packages = [
                rust
                pkgs."nodejs_${nodeVersion}"
                pkgs.jdk
              ];
              inputsFrom = [ inputs'.tauri-plugin-holochain.devShells.androidDev ];
            };

          devShells.iosDev =
            let
              rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.ios.toml;
            in
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
