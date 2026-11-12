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
          # The Erlang NIF, as lib/libnickel_embed_nif.so. gleam-nickel loads it
          # through NICKEL_EMBED_NIF.
          nickel-embed-nif = pkgs.rustPlatform.buildRustPackage {
            pname = "nickel-embed-nif";
            version = "0.1.0";
            src = pkgs.lib.fileset.toSource {
              root = ./.;
              fileset = pkgs.lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./README.md
                ./nickel-embed
                # Not built here, but a workspace member, so Cargo reads its manifest.
                ./nickel-embed-capi/Cargo.toml
                ./nickel-embed-capi/src
                ./nickel-embed-nif
              ];
            };
            cargoLock.lockFile = ./Cargo.lock;
            cargoBuildFlags = [
              "-p"
              "nickel-embed-nif"
            ];
            # Tested from Erlang: see checks.nickel-embed-nif.
            doCheck = false;
            # The Erlang VM loads `.so` files on every Unix, macOS included.
            postInstall = ''
              if [ -f $out/lib/libnickel_embed_nif.dylib ]; then
                mv $out/lib/libnickel_embed_nif.dylib $out/lib/libnickel_embed_nif.so
              fi
            '';
          };
          default = nickel-embed-nif;
        }
      );

      checks = forEachSupportedSystem (
        { pkgs, system }:
        {
          # The NIF's Erlang tests, against the packaged library.
          nickel-embed-nif =
            pkgs.runCommand "nickel-embed-nif-test" { nativeBuildInputs = [ pkgs.erlang ]; }
              ''
                cp ${./nickel-embed-nif/test}/*.erl .
                erlc *.erl
                export HOME=$TMPDIR
                NICKEL_EMBED_NIF=${self.packages.${system}.nickel-embed-nif}/lib/libnickel_embed_nif \
                  erl -noshell -pa . -eval \
                  'case eunit:test(nif_test, [verbose]) of ok -> halt(0); _ -> halt(1) end.'
                touch $out
              '';
        }
      );

      formatter = forEachSupportedSystem ({ pkgs, ... }: pkgs.nixfmt);
    };
}
