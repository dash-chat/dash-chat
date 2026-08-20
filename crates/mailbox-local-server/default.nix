{ inputs, self, ... }:

{
  perSystem = { inputs', pkgs, self', lib, system, ... }:
    let
      overlays = [ (import inputs.rust-overlay) ];
      craneLibPkgs = import inputs.nixpkgs { inherit system overlays; };
      rust = craneLibPkgs.rust-bin.fromRustupToolchainFile "${self}/rust-toolchain.toml";
      craneLib = (inputs.crane.mkLib craneLibPkgs).overrideToolchain rust;
      src = craneLib.cleanCargoSource (craneLib.path self.outPath);

      cratePath = ./.;

      cargoToml =
        builtins.fromTOML (builtins.readFile "${cratePath}/Cargo.toml");
      crate = cargoToml.package.name;
      pname = crate;
      version = cargoToml.package.version;

      commonArgs = {
        inherit src version pname;
        doCheck = false;
        buildInputs = [pkgs.openssl];
        nativeBuildInputs=[pkgs.pkg-config];
        cargoExtraArgs = "-p mailbox-local-server";
      };

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      appVersion = (builtins.fromJSON
        (builtins.readFile "${self}/src-tauri/tauri.conf.json")).version;

    in {
      packages = {
        mailbox-local-server =
          craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
      } // lib.optionalAttrs pkgs.stdenv.isLinux {

        local-message-server-docker = pkgs.dockerTools.streamLayeredImage {
          name = "ghcr.io/dash-chat/local-message-server";
          tag = appVersion;
          config = {
            Entrypoint = [
              "${self'.packages.mailbox-local-server}/bin/mailbox-local-server"
              "--addr"
              "[::]:3000"
              "--db-path"
              "/var/lib/local-message-server/mailbox.redb"
            ];
            ExposedPorts = { "3000/tcp" = { }; };
            Volumes = { "/var/lib/local-message-server" = { }; };
            Labels = {
              "org.opencontainers.image.source" =
                "https://github.com/dash-chat/dash-chat";
              "org.opencontainers.image.description" =
                "Local message server for Dash Chat: a LAN mailbox that holds messages for peers that are offline and announces itself over mDNS, so devices on the same network keep chatting with no cloud service and no internet access. Run it with `docker run --network host` so the mDNS announcement reaches the LAN; state lives in /var/lib/local-message-server.";
            };
          };
        };
      };
    };
}
