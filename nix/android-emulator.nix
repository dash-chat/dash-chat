{ inputs, ... }:

{
  perSystem =
    { lib, pkgs, system, ... }:
    {
      packages = lib.optionalAttrs (system == "x86_64-linux") (
        let
          # emulateApp needs the unfree Android SDK with its license accepted,
          # so it gets its own nixpkgs import instead of the shared one.
          emulator =
            (import inputs.nixpkgs {
              inherit system;
              config = {
                allowUnfree = true;
                android_sdk.accept_license = true;
              };
            }).androidenv.emulateApp
              {
                name = "dash-chat-e2e-emulator";
                platformVersion = "35";
                abiVersion = "x86_64";
                systemImageType = "google_apis";
              };
        in
        {
          android-emulator = emulator;
          # Self-contained headless launcher. run-test-emulator only exports
          # ANDROID_SDK_ROOT, but the emulator prefers ANDROID_HOME when a CI
          # runner pre-sets it to a foreign SDK — so point ANDROID_HOME at
          # the same nix SDK, extracted from the generated script.
          boot-emulator = pkgs.writeShellApplication {
            name = "boot-emulator";
            text = ''
              ANDROID_HOME="$(sed -n 's/^export ANDROID_SDK_ROOT=//p' ${emulator}/bin/run-test-emulator)"
              export ANDROID_HOME
              export NIX_ANDROID_EMULATOR_FLAGS="''${NIX_ANDROID_EMULATOR_FLAGS:--no-window -no-audio -no-boot-anim -gpu swiftshader_indirect}"
              exec ${emulator}/bin/run-test-emulator
            '';
          };
        }
      );
    };
}
