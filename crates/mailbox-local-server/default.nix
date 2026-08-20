{ inputs, self, ... }:

{
  perSystem =
    {
      inputs',
      pkgs,
      self',
      lib,
      system,
      ...
    }:
    let
      overlays = [ (import inputs.rust-overlay) ];
      craneLibPkgs = import inputs.nixpkgs { inherit system overlays; };
      rust = craneLibPkgs.rust-bin.fromRustupToolchainFile "${self}/rust-toolchain.toml";
      craneLib = (inputs.crane.mkLib craneLibPkgs).overrideToolchain rust;
      src = craneLib.cleanCargoSource (craneLib.path self.outPath);

      cratePath = ./.;

      cargoToml = builtins.fromTOML (builtins.readFile "${cratePath}/Cargo.toml");
      crate = cargoToml.package.name;
      pname = crate;
      version = cargoToml.package.version;

      commonArgs = {
        inherit src version pname;
        doCheck = false;
        buildInputs = [ pkgs.openssl ];
        nativeBuildInputs = [ pkgs.pkg-config ];
        cargoExtraArgs = "-p mailbox-local-server";
      };

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

      appVersion = (builtins.fromJSON (builtins.readFile "${self}/src-tauri/tauri.conf.json")).version;

    in
    {
      packages = {
        mailbox-local-server = craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
      }
      // lib.optionalAttrs pkgs.stdenv.isLinux {

        # Unlike the cloud servers in nix/docker.nix, this one is a LAN
        # appliance: it announces itself over mDNS, so it has to run with
        # `docker run --network host`. Multicast on 5353 does not cross a
        # bridge network, and the addresses the mDNS record auto-announces
        # would be the container's, unreachable from the LAN. Sharing the
        # host's network stack is also why it stays off port 80.
        local-message-server-docker = pkgs.dockerTools.streamLayeredImage {
          name = "ghcr.io/dash-chat/local-message-server";
          tag = appVersion;
          config = {
            # Entrypoint (not Cmd) so environment-specific args can be
            # appended at deploy time: docker run <image> <extra args>.
            # Binds dual-stack so peers reach the server over both the IPv4
            # and IPv6 addresses the mDNS record announces.
            Entrypoint = [
              "${self'.packages.mailbox-local-server}/bin/mailbox-local-server"
              "--addr"
              "[::]:3000"
              "--db-path"
              "/var/lib/local-message-server/mailbox.redb"
            ];
            ExposedPorts = {
              "3000/tcp" = { };
            };
            # All persistent state (redb + the iroh blob store beside it)
            # lives here; bind-mount a host directory onto it.
            Volumes = {
              "/var/lib/local-message-server" = { };
            };
            Labels = {
              "org.opencontainers.image.source" = "https://github.com/dash-chat/dash-chat";
            };
          };
        };
      };
    };
}
