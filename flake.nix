{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs = { self, nixpkgs }: let
    pkgs = nixpkgs.legacyPackages."x86_64-linux";
  in {
    devShells."x86_64-linux".default = pkgs.mkShell {
      buildInputs = with pkgs; [
        cargo rustc rustfmt rust-analyzer
        alsa-lib.dev
      ];

      nativeBuildInputs = [ pkgs.pkg-config ];

      env.RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";

      shellHook = ''
        if [ -n "$ZSH_VERSION" ]; then
          eval "$(oh-my-posh init zsh --config $HOME/.config/oh-my-posh/theme.toml)"
        elif [ -n "$BASH_VERSION" ]; then
          eval "$(oh-my-posh init bash --config $HOME/.config/oh-my-posh/theme.toml)"
        fi
      '';
    };
  };
}
