{
  description = "zero-meet Mac build tools";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { nixpkgs, ... }:
    let
      systems = [ "aarch64-darwin" "x86_64-darwin" ];
    in
    {
      devShells = nixpkgs.lib.genAttrs systems (system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = pkgs.mkShellNoCC {
            packages = with pkgs; [
              cargo
              rustc
              rustfmt
              nodejs_24
              pkg-config
              cmake
            ];

            # Use Apple's installed SDK and linker for native Mac builds.
            shellHook = ''
              export SDKROOT="$(xcrun --sdk macosx --show-sdk-path)"
              export LIBRARY_PATH="$SDKROOT/usr/lib''${LIBRARY_PATH:+:$LIBRARY_PATH}"
            '';
          };
        });
    };
}
