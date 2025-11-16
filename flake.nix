{
  description = "Devenv with QT + Rust bindings";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };

  };

  outputs = { self, nixpkgs,
    # nix-filter, 
    ... }:
    let
      inherit (nixpkgs) lib;
      systems = lib.systems.flakeExposed;

      forAllSystems = lib.genAttrs systems;
      nixpkgsFor = forAllSystems (system: nixpkgs.legacyPackages.${system});
    in {
      devShells = forAllSystems (system:
        let
          pkgs = nixpkgsFor.${system};

          llvm' = pkgs.llvmPackages_21;

          libPath = lib.makeLibraryPath [ ];

          inherit (pkgs.stdenv.hostPlatform) isLinux;
          inherit (pkgs.qt6Packages) qtbase qtwayland wrapQtAppsHook;

          qt-wrapper-env = pkgs.stdenv.mkDerivation {
            name = "qt-wrapper-env";

            nativeBuildInputs = [ pkgs.makeWrapper wrapQtAppsHook ];

            buildInputs = [ pkgs.qt6Packages.qtbase ]
              ++ lib.optional isLinux qtwayland;

            buildCommand = ''
              makeQtWrapper ${lib.getExe pkgs.runtimeShellPackage} "$out"
              sed -i '/^exec/d' "$out" 
            '';
          };

          qt-plugin-path = (lib.strings.concatStringsSep ":"
            (builtins.map (p: "${p}/lib/qt-6/plugins")
              ([ qtbase ] ++ lib.optional isLinux qtwayland)));
        in {
          default = pkgs.mkShell {
            name = "tokio-ffi-test";

            nativeBuildInputs = with pkgs; [ cmake ninja ];

            buildInputs = with pkgs;
              [
                qtbase
                zlib
                # hematite
                ccache
                ninja
                cargo
                rustc
                rust-analyzer
                rustfmt
                clippy
                llvm'.clang-tools
                rustPlatform.bindgenHook
              ] ++ lib.optional isLinux qtwayland;

            LD_LIBRARY_PATH = libPath;

            cmakeBuildType = "Debug";
            cmakeFlags = [ "-GNinja" ];
            dontFixCmake = true;

            shellHook = ''
              echo "Sourcing ${qt-wrapper-env}"
              source ${qt-wrapper-env}

              echo "Patching QT_PLUGIN_PATH"
              export QT_PLUGIN_PATH="${qt-plugin-path}:$QT_PLUGIN_PATH"

              if [ ! -f compile_commands.json ]; then
                cmakeConfigurePhase
                cd ..
                ln -s "$cmakeBuildDir"/compile_commands.json compile_commands.json
              fi

            '';
          };
        });
    };
}
