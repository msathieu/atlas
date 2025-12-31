{
  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixos-25.11";
  outputs = { self, nixpkgs }:
    let pkgs = nixpkgs.legacyPackages.x86_64-linux;
    in {
      devShells.x86_64-linux.default = pkgs.mkShell {
        packages = with pkgs; [
          apksigner
          cargo-deny
          deno
          diesel-cli
          lua5_4
          openjdk
          openssl
          pkg-config
          postgresql
          protobuf
          rustup
          sdkmanager
          sqlite-interactive
        ];
        shellHook = ''
          export ANDROID_HOME=$HOME/Android
        '';
      };
    };
}
