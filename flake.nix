{
  description = "nickel-embed: embed Nickel in Rust";

  inputs = {
    nixpkgs.url = "https://flakehub.com/f/NixOS/nixpkgs/0.1"; # unstable Nixpkgs
    fenix = {
      url = "https://flakehub.com/f/nix-community/fenix/0.1";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { self, ... }@inputs:

    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forEachSupportedSystem =
        f:
        inputs.nixpkgs.lib.genAttrs supportedSystems (
          system:
          f {
            inherit system;
            pkgs = import inputs.nixpkgs {
              inherit system;
              overlays = [
                inputs.self.overlays.default
              ];
            };
          }
        );
    in
    {
      overlays.default = final: prev: {
        rustToolchain =
          with inputs.fenix.packages.${prev.stdenv.hostPlatform.system};
          combine (
            with stable;
            [
              clippy
              rustc
              cargo
              rustfmt
              rust-src
            ]
          );
      };

      devShells = forEachSupportedSystem (
        { pkgs, system }:
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustToolchain
              openssl
              pkg-config
              cargo-deny
              cargo-edit
              cargo-watch
              rust-analyzer
              self.formatter.${system}
            ];

            env = {
              # Required by rust-analyzer
              RUST_SRC_PATH = "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";
            };
          };
        }
      );

      packages = forEachSupportedSystem (
        { pkgs, ... }:
        rec {
          # The C library (lib/libnickel_lang.a and .so) and its header. The language
          # bindings live in their own repositories: see docs/bindings.md.
          nickel-embed-capi = pkgs.rustPlatform.buildRustPackage {
            pname = "nickel-embed-capi";
            version = "0.1.0";
            src = pkgs.lib.fileset.toSource {
              root = ./.;
              fileset = pkgs.lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./README.md
                ./nickel-embed
                ./nickel-embed-capi
              ];
            };
            cargoLock.lockFile = ./Cargo.lock;
            # rustls's crypto (aws-lc-sys), for fetching packages over https. nixpkgs's
            # Cargo setup asks it to link a system AWS-LC (AWS_LC_SYS_USE_SYSTEM=1), and there
            # is none here; the target-specific variable wins over that, so it builds its own
            # copy from source.
            env."AWS_LC_SYS_USE_SYSTEM_${
              builtins.replaceStrings [ "-" ] [ "_" ] pkgs.stdenv.hostPlatform.rust.rustcTarget
            }" =
              "0";
            # The `doc` feature's syntax highlighting links oniguruma, which nixpkgs's Cargo
            # setup tells onig_sys to take from the system (RUSTONIG_SYSTEM_LIBONIG).
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.oniguruma ];
            cargoBuildFlags = [
              "-p"
              "nickel-embed-capi"
            ];
            # Tested by `cargo test` and the C tests in CI.
            doCheck = false;
            postInstall = ''
              mkdir -p $out/include
              cp nickel-embed-capi/include/nickel_lang.h $out/include/
            '';
          };
          default = nickel-embed-capi;
        }
      );

      formatter = forEachSupportedSystem ({ pkgs, ... }: pkgs.nixfmt);
    };
}
