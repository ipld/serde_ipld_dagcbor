{
  description = "serde_ipld_dagcbor";

  inputs = {
    nixpkgs.url = "nixpkgs/nixos-23.11";
    nixos-unstable.url = "nixpkgs/nixos-unstable-small";

    command-utils.url = "github:expede/nix-command-utils";
    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
    };
  };

  outputs = {
      self,
      nixpkgs,
      nixos-unstable,
      command-utils,
      flake-utils,
      rust-overlay,
  } @ inputs:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            (import rust-overlay)
          ];
        };

        unstable = import nixos-unstable {
          inherit system;
        };

        rust-toolchain =
          pkgs.rust-bin.stable.latest.default.override {
            extensions = [
              "cargo"
              "clippy"
              "rust-src"
              "rust-std"
              "rustfmt"
            ];
          };

        nightly-rustfmt = pkgs.rust-bin.nightly.latest.rustfmt;

        format-pkgs = [
          pkgs.nixpkgs-fmt
          pkgs.alejandra
        ];

        darwin-installs = [
          pkgs.darwin.apple_sdk.frameworks.Security
          pkgs.darwin.apple_sdk.frameworks.CoreFoundation
          pkgs.darwin.apple_sdk.frameworks.Foundation
        ];

        cargo-installs = [
          pkgs.cargo-expand
          pkgs.cargo-outdated
          pkgs.cargo-sort
          pkgs.cargo-udeps
          pkgs.cargo-watch
        ];

        cargo = "${pkgs.cargo}/bin/cargo";

        cmd = command-utils.cmd.${system};

        release = {
          "release" = cmd "Build release for ${system}"
            "${cargo} build --release";
        };

        build = {
          "build" = cmd "Build for ${system}"
            "${cargo} build";
        };

        lint = {
          "lint" = cmd "Run Clippy"
            "${cargo} clippy";

          "lint:pedantic" = cmd "Run Clippy pedantically"
            "${cargo} clippy -- -W clippy::pedantic";

          "lint:fix" = cmd "Apply non-pendantic Clippy suggestions"
            "${cargo} clippy --fix";
        };

        watch = {
          "watch:build" = cmd "Rebuild host target on save"
            "${cargo} watch --clear";

          "watch:lint" = cmd "Lint on save"
            "${cargo} watch --clear --exec clippy";

          "watch:lint:pedantic" = cmd "Pedantic lint on save"
            "${cargo} watch --clear --exec 'clippy -- -W clippy::pedantic'";

          "watch:test" = cmd "Run all host tests on save"
            "${cargo} watch --clear --exec test";
        };

        test = {
          "tests" = cmd "Run Cargo tests for host target"
            "${cargo} test";

          "tests:docs" = cmd "Run Cargo doctests"
            "${cargo} test --doc";
        };

        docs = {
          "docs" = cmd "Refresh the docs"
            "${cargo} doc";

          "docs:open" = cmd "Open refreshed docs"
            "${cargo} doc --open";
        };

        command_menu = command-utils.commands.${system}
          (release // build // lint // watch // test // docs);

      in rec {
        devShells.default = pkgs.mkShell {
          name = "serde_ipld_dagcbor";

          nativeBuildInputs = [
            rust-toolchain
            self.packages.${system}.irust
            (pkgs.hiPrio pkgs.rust-bin.nightly.latest.rustfmt)
            command_menu
          ]
          ++ format-pkgs
          ++ cargo-installs
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin darwin-installs;

          shellHook = ''
            unset SOURCE_DATE_EPOCH
            menu    
          '';
        };

        formatter = pkgs.alejandra;

        packages.irust = pkgs.rustPlatform.buildRustPackage rec {
          pname = "irust";
          version = "1.71.19";
          src = pkgs.fetchFromGitHub {
            owner = "sigmaSd";
            repo = "IRust";
            rev = "irust@${version}";
            sha256 = "sha256-R3EAovCI5xDCQ5R69nMeE6v0cGVcY00O3kV8qHf0akc=";
          };

          doCheck = false;
          cargoSha256 = "sha256-2aVCNz/Lw7364B5dgGaloVPcQHm2E+b/BOxF6Qlc8Hs=";
        };
      }
    );
}
