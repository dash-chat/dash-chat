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

  push_notifications_server_module = {
    systemd.services.push-notifications =
      let push = self.outputs.packages."x86_64-linux".push-notifications-server;
      in {
        enable = true;
        path = [ push ];
        wantedBy = [ "multi-user.target" ];
        after = [ "network-online.target" ];
        wants = [ "network-online.target" ];
        serviceConfig = {
          ExecStart =
            "${push}/bin/push-notifications-server --addr 0.0.0.0:80 --service-account-key /var/lib/push-notifications/service-account-key.json --db-path /var/lib/push-notifications/push-notifications.sqlite";
          Restart = "always";
          StateDirectory = "push-notifications";
        };
      };
    networking.firewall.allowedTCPPorts = [ 80 ];
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
          ExecStart = "${mailbox}/bin/mailbox-server --addr 0.0.0.0:80 --push-notifications-url https://push-notifications-server.production.dash-chat.dash-chat.garnix.me";
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

      push-notifications-server = inputs.nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
          inputs.garnix-lib.nixosModules.garnix
          sshModule
          push_notifications_server_module
          {
            garnix.server.persistence.name = "dash-chat-push-notifications";
            system.stateVersion = "25.05";
            garnix.server.enable = true;
            garnix.server.persistence.enable = true;
          }
        ];
      };
    };

  };

}

