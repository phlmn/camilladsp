let
  crossPkgs = import <nixpkgs> {
    crossSystem = {
      config = "aarch64-unknown-linux-gnu";
    };
  };
  pkgs = import <nixpkgs> {};
in
  crossPkgs.callPackage (
    {pkg-config, mkShell, alsa-lib, gcc}:
    mkShell {
      nativeBuildInputs = [ gcc pkg-config ];
      buildInputs = [ alsa-lib ];
    }
  ) {}
