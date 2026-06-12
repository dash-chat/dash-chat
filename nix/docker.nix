{ self, ... }:

{
  perSystem =
    {
      pkgs,
      self',
      lib,
      ...
    }:
    let
      appVersion = (builtins.fromJSON (builtins.readFile "${self}/src-tauri/tauri.conf.json")).version;

      # Minimal images: just the server binary and CA certs, no shell.
      # All persistent state lives in /var/lib/<name> — the same path the
      # NixOS droplet images use — bind-mount a host directory onto it
      # (docker run -v /host/dir:/var/lib/<name> ...).
      mkServerImage =
        {
          name,
          package,
          cmdArgs,
        }:
        pkgs.dockerTools.streamLayeredImage {
          name = "ghcr.io/dash-chat/${name}";
          tag = appVersion;
          config = {
            Cmd = [
              "${package}/bin/${name}"
              "--addr"
              "0.0.0.0:80"
            ]
            ++ cmdArgs;
            Env = [ "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" ];
            ExposedPorts = {
              "80/tcp" = { };
            };
            Volumes = {
              "/var/lib/${name}" = { };
            };
            Labels = {
              "org.opencontainers.image.source" = "https://github.com/dash-chat/dash-chat";
            };
          };
        };
    in
    {
      packages = lib.optionalAttrs pkgs.stdenv.isLinux {
        mailbox-server-docker = mkServerImage {
          name = "mailbox-server";
          package = self'.packages.mailbox-server;
          cmdArgs = [
            "--db-path"
            "/var/lib/mailbox-server/mailbox.redb"
            "--push-notifications-url"
            "http://push-notifications.darksoil.studio"
          ];
        };

        push-notifications-server-docker = mkServerImage {
          name = "push-notifications-server";
          package = self'.packages.push-notifications-server;
          cmdArgs = [
            "--service-account-key"
            "/var/lib/push-notifications-server/service-account-key.json"
            "--db-path"
            "/var/lib/push-notifications-server/push-notifications.sqlite"
          ];
        };
      };
    };
}
