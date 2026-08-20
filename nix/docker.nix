{ self, inputs, ... }:

{
  perSystem = { pkgs, self', lib, ... }:
    let
      appVersion = (builtins.fromJSON
        (builtins.readFile "${self}/src-tauri/tauri.conf.json")).version;

      # Minimal images: just the server binary and CA certs, no shell.
      # All persistent state lives in /var/lib/<name> — the same path the
      # NixOS droplet images use — bind-mount a host directory onto it
      # (docker run -v /host/dir:/var/lib/<name> ...).
      mkServerImage = { name, package, cmdArgs, description }:
        pkgs.dockerTools.streamLayeredImage {
          name = "ghcr.io/dash-chat/${name}";
          tag = appVersion;
          config = {
            # Entrypoint (not Cmd) so environment-specific args can be
            # appended at deploy time: docker run <image> <extra args>.
            Entrypoint = [ "${package}/bin/${name}" "--addr" "0.0.0.0:80" ]
              ++ cmdArgs;
            Env =
              [ "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt" ];
            ExposedPorts = { "80/tcp" = { }; };
            Volumes = { "/var/lib/${name}" = { }; };
            Labels = {
              "org.opencontainers.image.source" =
                "https://github.com/dash-chat/dash-chat";
              "org.opencontainers.image.description" = description;
            };
          };
        };
    in {
      packages = lib.optionalAttrs pkgs.stdenv.isLinux {
        mailbox-server-docker = mkServerImage {
          name = "mailbox-server";
          package = self'.packages.mailbox-server;
          cmdArgs = [ "--db-path" "/var/lib/mailbox-server/mailbox.redb" ];
          description =
            "Mailbox server for Dash Chat: holds end-to-end encrypted messages for peers that are offline until they reconnect. State lives in /var/lib/mailbox-server; bind-mount a host directory onto it to persist messages across restarts.";
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
          description =
            "Push notifications server for Dash Chat: registers device tokens per topic and dispatches FCM notifications when a mailbox server reports new messages. Needs a Google service account key at /var/lib/push-notifications-server/service-account-key.json.";
        };
      };
    };
}
