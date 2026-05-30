{ config, lib, ... }:

let
  cfg = config.programs.signal-auras.unsafeInput;

  deviceType = lib.types.submodule {
    options = {
      id = lib.mkOption {
        type = lib.types.strMatching "[A-Za-z0-9._-]+";
        description = ''
          Stable identifier used for /dev/input/by-signal-auras/<id>.
        '';
      };

      match = lib.mkOption {
        type = lib.types.lines;
        example = ''ATTRS{name}=="Example Keyboard"'';
        description = ''
          Udev match fragment for one selected evdev device. Use only match
          predicates such as ATTRS{name}, ATTRS{phys}, ENV{ID_PATH}, or
          ENV{ID_SERIAL}; action assignments are rejected.
        '';
      };
    };
  };

  hasDangerousUdevAction = fragment:
    lib.any (needle: lib.hasInfix needle fragment) [
      "RUN+="
      "PROGRAM="
      "IMPORT{program}"
      "IMPORT{builtin}"
    ];

  selectedDeviceRule = device: ''
    SUBSYSTEM=="input", KERNEL=="event*", ${device.match}, GROUP="${cfg.group}", MODE="0640", SYMLINK+="input/by-signal-auras/${device.id}"
  '';

  uinputRule = ''
    KERNEL=="uinput", GROUP="${cfg.group}", MODE="0660", OPTIONS+="static_node=uinput"
  '';
in
{
  options.programs.signal-auras.unsafeInput = {
    enable = lib.mkEnableOption ''
      persistent selected-device evdev and uinput permissions for Signal Auras
    '';

    users = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ "aron" ];
      description = ''
        Users added to the Signal Auras input group. Group membership normally
        requires a new login session before it affects already-running shells.
      '';
    };

    group = lib.mkOption {
      type = lib.types.str;
      default = "signal-auras-input";
      description = "Group granted access to the selected input devices.";
    };

    selectedDevices = lib.mkOption {
      type = lib.types.listOf deviceType;
      default = [ ];
      description = ''
        Selected evdev devices exposed to Signal Auras. Each entry creates a
        stable /dev/input/by-signal-auras/<id> symlink and grants read access
        to the configured group.
      '';
    };

    uinput.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Grant the Signal Auras input group read/write access to /dev/uinput and
        load the uinput kernel module at boot.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.selectedDevices != [ ] || cfg.uinput.enable;
        message = "programs.signal-auras.unsafeInput requires selectedDevices or uinput.enable.";
      }
      {
        assertion =
          let ids = map (device: device.id) cfg.selectedDevices;
          in lib.length ids == lib.length (lib.unique ids);
        message = "programs.signal-auras.unsafeInput.selectedDevices ids must be unique.";
      }
      {
        assertion = lib.all (device: !(hasDangerousUdevAction device.match)) cfg.selectedDevices;
        message = "Signal Auras selected-device udev matches must not contain RUN, PROGRAM, or IMPORT actions.";
      }
    ];

    users.groups.${cfg.group} = { };
    users.users = lib.genAttrs cfg.users (_user: {
      extraGroups = [ cfg.group ];
    });

    boot.kernelModules = lib.mkIf cfg.uinput.enable [ "uinput" ];

    services.udev.extraRules = lib.mkAfter (lib.concatStringsSep "\n" (
      (map selectedDeviceRule cfg.selectedDevices)
      ++ lib.optional cfg.uinput.enable uinputRule
    ));
  };
}
