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
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            bash
            cargo
            direnv
            git
            just
            pkg-config
            python313
            rustc
            rustfmt
            clippy
            uv
            wayland
            wayland-protocols
          ];

          shellHook = ''
            export SPECKIT_VERSION="v0.8.13"
            export UV_TOOL_BIN_DIR="$PWD/.direnv/uv/bin"
            export UV_TOOL_DIR="$PWD/.direnv/uv/tools"
            export UV_CACHE_DIR="$PWD/.direnv/uv/cache"
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
