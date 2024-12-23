{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.moq-relay;
in
{
  options.services.moq-relay = {
    enable = lib.mkEnableOption "moq-relay";
    dev = lib.mkEnableOption "dev";
    port = lib.mkOption {
      type = lib.types.port;
      default = 443;
      description = "Relay server port";
    };
    user = lib.mkOption {
      type = with lib.types; uniq str;
      description = ''
        User account that runs moq-relay.

        ::: {.note}
        This user must have access to the TLS certificate and key.
        :::
      '';
    };
    group = lib.mkOption {
      type = with lib.types; uniq str;
      description = ''
        Group account that runs moq-relay.

        ::: {.note}
        This group must have access to the TLS certificate and key.
        :::
      '';
    };
    tls = {
      keyPath = lib.mkOption {
        type = lib.types.path;
        example = "/var/lib/acme/moq-relay.example.org/key.pem";
        description = ''
          Path to TLS private key
        '';
      };
      certPath = lib.mkOption {
        type = lib.types.path;
        example = "/var/lib/acme/moq-relay.example.org/cert.pem";
        description = ''
          Path to TLS certificate
        '';
      };
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.moq-relay = {
      description = "Media over QUIC relay server";
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        User = cfg.user;
        Group = cfg.group;
        ExecStart = builtins.concatStringsSep " " ([
          "${pkgs.moq.moq-relay}/bin/moq-relay --bind [::]:${builtins.toString cfg.port} --tls-cert ${cfg.tls.certPath} --tls-key ${cfg.tls.keyPath}"
        ] ++ lib.optionals cfg.dev [ "--dev" ]);
        Restart = "on-failure";
        RestartSec = "1";

        # hardening
        RemoveIPC = true;
        CapabilityBoundingSet = [ "" ];
        DynamicUser = true;
        NoNewPrivileges = true;
        PrivateDevices = true;
        ProtectClock = true;
        ProtectKernelLogs = true;
        ProtectControlGroups = true;
        ProtectKernelModules = true;
        SystemCallArchitectures = "native";
        SystemCallFilter = "@system-service";
        RestrictNamespaces = true;
        RestrictSUIDSGID = true;
        ProtectHostname = true;
        LockPersonality = true;
        ProtectKernelTunables = true;
        RestrictAddressFamilies = [
          "AF_INET"
          "AF_INET6"
          "AF_UNIX"
        ];
        RestrictRealtime = true;
        ProtectSystem = "strict";
        ProtectProc = "invisible";
        ProcSubset = "pid";
        ProtectHome = true;
        PrivateUsers = true;
        PrivateTmp = true;
      };
    };
  };
}
