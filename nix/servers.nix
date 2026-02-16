{ self, inputs, ... }:
let

  sshPubKeys = {
    guillem =
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIDTE+RwRfcG3UNTOZwGmQOKd5R+9jN0adH4BIaZvmWjO guillem.cordoba@gmail.com";
    guillemslaptop =
      "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIO8DVpvRgQ90MyMyiuNdvyMNAio9n2o/+57MyhZS2A5A guillem.cordoba@gmail.com";
  };

  sshModule = {
    users.users.root.openssh.authorizedKeys.keys =
      builtins.attrValues sshPubKeys;
    services.openssh.enable = true;
    services.openssh.settings.PermitRootLogin = "without-password";
  };

  mailbox_server_module = {
    systemd.services.mailbox =
      let mailbox = self.outputs.packages."x86_64-linux".mailbox-server;
      in {
        enable = true;
        path = [ mailbox ];
        wantedBy = [ "multi-user.target" ];
        after = [ "network-online.target" ];
        wants = [ "network-online.target" ];
        serviceConfig = {
          ExecStart = "${mailbox}/bin/mailbox-server --addr 0.0.0.0:80";
          Restart = "always";
        };
      };
    networking.firewall = {
      enable = true;
      allowedTCPPorts = [ 80 ];
    };
  };

in {
  flake = {
    nixosConfigurations = {
      mailbox-server = inputs.nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          inputs.garnix-lib.nixosModules.garnix
          sshModule
          mailbox_server_module
          {
            garnix.server.persistence.name = "dash-chat-mailbox";
            system.stateVersion = "25.05";
            garnix.server.enable = true;
            garnix.server.persistence.enable = true;
          }
        ];
      };
    };
  };
}

