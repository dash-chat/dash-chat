{ self, inputs, ... }:

let
  # Appliance images: deployed by uploading a fresh image per release, never
  # rebuilt in place, so nix, nixos-rebuild and the nixpkgs source are omitted.
  minimalModule = { modulesPath, ... }: {
    imports = [ "${modulesPath}/profiles/minimal.nix" ];
    nixpkgs.flake.setNixPath = false;
    nixpkgs.flake.setFlakeRegistry = false;
    system.disableInstallerTools = true;
    virtualisation.digitalOcean.rebuildFromUserData = false;
    nix.enable = false;
    services.do-agent.enable = false;

    # grow-partition only needs growpart from cloud-utils; ec2metadata's
    # shebang would drag all of python3 (~106MiB) into the image.
    nixpkgs.overlays = [
      (final: prev: {
        cloud-utils = prev.cloud-utils.overrideAttrs (old: {
          postFixup = old.postFixup + ''
            rm $guest/bin/ec2metadata $guest/bin/.ec2metadata-wrapped $out/bin/ec2metadata
          '';
        });
      })
    ];
  };

  # Calls make-disk-image directly instead of using
  # config.system.build.digitalOceanImage, which hardcodes copyChannel = true
  # and would bake a ~186MiB nixpkgs channel copy into the image.
  mkDoImage = modules:
    let
      sys = inputs.nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        specialArgs = { inherit self inputs; };
        modules = modules ++ [
          "${inputs.nixpkgs}/nixos/modules/virtualisation/digital-ocean-image.nix"
          minimalModule
          { system.stateVersion = "25.05"; }
        ];
      };
    in
    import "${inputs.nixpkgs}/nixos/lib/make-disk-image.nix" {
      name = "digital-ocean-image";
      inherit (sys) config pkgs;
      inherit (sys.config.image) baseName;
      inherit (sys.config.virtualisation) diskSize;
      lib = inputs.nixpkgs.lib;
      # Internally-compressed qcow2: DO's upload UI rejects .gz files but
      # accepts this, and it stays small without external compression.
      format = "qcow2-compressed";
      copyChannel = false;
      configFile = sys.config.virtualisation.digitalOcean.defaultConfigFile;
    };

in {
  flake.packages.x86_64-linux = {
    mailbox-server-do-image = mkDoImage [
      self.nixosModules.ssh
      self.nixosModules.mailbox-server
    ];

    push-notifications-server-do-image = mkDoImage [
      self.nixosModules.ssh
      self.nixosModules.push-notifications-server
    ];
  };
}
