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
        # Builds the headless `local-mailbox-server` daemon binary.
        cargoExtraArgs = "-p replicating-local-mailbox-server";
      };

      cargoArtifacts = craneLib.buildDepsOnly commonArgs;

    in {
      packages.replicating-local-mailbox-server =
        craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
    };
}
