{ self, inputs, ... }:

let
  mkDoImage = modules:
    (inputs.nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      specialArgs = { inherit self inputs; };
      modules = modules ++ [
        "${inputs.nixpkgs}/nixos/modules/virtualisation/digital-ocean-image.nix"
        { system.stateVersion = "25.05"; }
      ];
    }).config.system.build.digitalOceanImage;

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
