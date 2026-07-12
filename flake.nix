{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { self, nixpkgs, fenix }:
    let pkgs = nixpkgs.legacyPackages.x86_64-linux;
    in {
      devShells.x86_64-linux.default = pkgs.mkShell {
        packages = with pkgs; [
          apksigner
          cargo-deny
          cmake
          deno
          diesel-cli
          git
          jq
          kubectl
          lua5_5
          openjdk
          openssl
          pkg-config
          postgresql_18
          protobuf
          sdkmanager
          sqlite-interactive
          fenix.packages.x86_64-linux.stable.toolchain
        ];
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.openssl pkgs.libpq ];
        shellHook = ''
          export ANDROID_HOME=$HOME/Android
        '';
      };
    };
}
