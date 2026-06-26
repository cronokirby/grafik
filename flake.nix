
{
  description = "grafik Rust workspace";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    flake-utils.url = "github:numtide/flake-utils";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      naersk,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };
        naersk' = pkgs.callPackage naersk { };
        guiRuntimeLibs = with pkgs; [
          libGL
          libx11
          libxcb
          libxcursor
          libxi
          libxkbcommon
          libxrandr
          vulkan-loader
          wayland
        ];
      in
      {
        packages.grafik-gui = naersk'.buildPackage {
          name = "grafik-app";
          pname = "grafik-app";
          version = "0.1.0";
          root = ./.;
          cargoBuildOptions = old: old ++ [ "-p" "grafik-app" ];
        };

        packages.default = self.packages.${system}.grafik-gui;

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            cargo
            clippy
            rust-analyzer
            rustc
            rustfmt
            wgsl-analyzer
            just
          ];

          buildInputs = guiRuntimeLibs;

          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath guiRuntimeLibs;

          RUST_BACKTRACE = "1";
        };
      }
    );
}
