{
  description = "Prismlauncher libprism Rust bindings";

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
          libPath = lib.makeLibraryPath [ ];
        in {
          default = pkgs.mkShell {
            name = "tokio-ffi-test";

            nativeBuildInputs = with pkgs; [ cmake ninja ];

            buildInputs = with pkgs; [
              qt6Packages.qtbase
              zlib
              # hematite
              ccache
              ninja
              llvmPackages.bintools
              llvmPackages.clang-tools
              cargo
              rustc
              rust-analyzer
              rustfmt
              clippy
            ];

            LD_LIBRARY_PATH = libPath;

            # # Add glibc, clang, glib, and other headers to bindgen search path
            BINDGEN_EXTRA_CLANG_ARGS =
              # Includes normal include path
              (builtins.map (a: ''-I"${a}/include"'') [
                # add dev libraries here (e.g. pkgs.libvmi.dev)
                pkgs.glibc.dev
              ])
              # Includes with special directory paths
              ++ [
                ''
                  -I"${pkgs.llvmPackages.libclang.lib}/lib/clang/${pkgs.llvmPackages_19.libclang.version}/include"''
                ''-I"${pkgs.glib.dev}/include/glib-2.0"''
                "-I${pkgs.glib.out}/lib/glib-2.0/include/"
              ];

            # CPLUS_INCLUDE_PATH = (builtins.map (a: ''"${a}/include"'') [
            #   # add dev libraries here (e.g. pkgs.libvmi.dev)
            #   pkgs.glibc.dev
            # ])
            # # Includes with special directory paths
            #   ++ [
            #     ''
            #       "${pkgs.llvmPackages.libclang.lib}/lib/clang/${pkgs.llvmPackages.libclang.version}/include"''
            #     ''"${pkgs.glib.dev}/include/glib-2.0"''
            #     "${pkgs.glib.out}/lib/glib-2.0/include/"
            #   ];
            #
            # CXXFLAGS = (builtins.map (a: ''-isystem "${a}/include"'') [
            #   pkgs.stdenv.cc.cc.lib
            #   pkgs.glibc.dev
            # ]) ++ [
            #   # ''
            #   #   "-isystem ${pkgs.llvmPackages.libclang.lib}/lib/clang/${pkgs.llvmPackages.libclang.version}/include"''
            #   ''-isystem "${pkgs.glib.dev}/include/glib-2.0"''
            #   ''-isystem "${pkgs.glib.out}/lib/glib-2.0/include/"''
            # ];

            shellHook = "";
          };
        });
    };
}
