{ inputs, ... }:

{
  perSystem = { lib, system, ... }: {
    packages = lib.optionalAttrs (system == "x86_64-linux") {
      # emulateApp needs the unfree Android SDK with its license accepted,
      # so it gets its own nixpkgs import instead of the shared one.
      android-emulator =
        (import inputs.nixpkgs {
          inherit system;
          config = {
            allowUnfree = true;
            android_sdk.accept_license = true;
          };
        }).androidenv.emulateApp {
          name = "dash-chat-e2e-emulator";
          platformVersion = "35";
          abiVersion = "x86_64";
          systemImageType = "google_apis";
        };
    };
  };
}
