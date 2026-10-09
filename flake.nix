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
          diesel-cli
          git
          jq
          kubectl
          kubernetes-helm
          lua5_5
          nodejs_26
          openjdk
          openssl
          pkg-config
          postgresql_18
          protobuf
          python3
          sdkmanager
          sqlite-interactive
          tpm2-tss
          fenix.packages.x86_64-linux.stable.toolchain
        ];
        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [ pkgs.openssl pkgs.libpq pkgs.tpm2-tss ];
        shellHook = ''
          export ANDROID_HOME=$HOME/Android
        '';
      };
    };
}
