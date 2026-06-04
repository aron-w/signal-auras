{
  description = "Development shell for signal-auras";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = { nixpkgs, ... }:
    let
      supportedSystems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];

      forAllSystems = function:
        nixpkgs.lib.genAttrs supportedSystems
          (system: function nixpkgs.legacyPackages.${system});
    in
    {
      nixosModules = {
        default = ./nixos/signal-auras.nix;
        signal-auras = ./nixos/signal-auras.nix;
      };

      checks = forAllSystems (pkgs:
        pkgs.lib.optionalAttrs pkgs.stdenv.isLinux
          (let
            moduleEval = nixpkgs.lib.nixosSystem {
              system = pkgs.stdenv.hostPlatform.system;
              modules = [
                ./nixos/signal-auras.nix
                {
                  programs.signal-auras.unsafeInput = {
                    enable = true;
                    users = [ "alice" ];
                    selectedDevices = [
                      {
                        id = "keyboard";
                        match = ''ATTRS{name}=="Example Keyboard"'';
                      }
                    ];
                  };
                }
              ];
            };
          in
          {
            nixos-module = pkgs.runCommand "signal-auras-nixos-module" {
              rules = moduleEval.config.services.udev.extraRules;
            } ''
              grep -q 'by-signal-auras/keyboard' <<< "$rules"
              grep -q 'KERNEL=="uinput"' <<< "$rules"
              touch "$out"
            '';
          }));

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            bash
            cargo
            direnv
            git
            imagemagick
            just
            lua-language-server
            llvmPackages.libclang
            pkg-config
            python313
            rustc
            rustfmt
            clippy
            uv
            wayland
            wayland-protocols
            dbus
            pipewire
            pipewire.dev
            xdg-desktop-portal
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            kdePackages.kglobalaccel
            kdePackages.kwin
            kdePackages.qttools
            kdePackages.xdg-desktop-portal-kde
            systemd
          ];

          shellHook = ''
            export SPECKIT_VERSION="v0.8.13"
            export UV_TOOL_BIN_DIR="$PWD/.direnv/uv/bin"
            export UV_TOOL_DIR="$PWD/.direnv/uv/tools"
            export UV_CACHE_DIR="$PWD/.direnv/uv/cache"
            export LIBCLANG_PATH="${pkgs.lib.getLib pkgs.llvmPackages.libclang}/lib"
            export BINDGEN_EXTRA_CLANG_ARGS="$(
              cat ${pkgs.stdenv.cc}/nix-support/libc-crt1-cflags
              cat ${pkgs.stdenv.cc}/nix-support/libc-cflags
              cat ${pkgs.stdenv.cc}/nix-support/cc-cflags
            )"
            export PATH="$UV_TOOL_BIN_DIR:$PATH"

            if [ ! -x "$UV_TOOL_BIN_DIR/specify" ]; then
              echo "Installing Spec Kit $SPECKIT_VERSION into $UV_TOOL_DIR"
              uv tool install specify-cli \
                --from "git+https://github.com/github/spec-kit.git@$SPECKIT_VERSION"
            fi
          '';
        };
      });
    };
}
