{
  description = "Gitar Nix integration";

  inputs = {
    nixpkgs.url      = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        cargoManifest = (pkgs.lib.importTOML ./Cargo.toml).package;
        rustPlatform = pkgs.makeRustPlatform {
          cargo = pkgs.rust-bin.stable.latest.minimal;
          rustc = pkgs.rust-bin.stable.latest.minimal;
        };
        gitcleanbranches = pkgs.writeScriptBin "git-clean-branches" ''
          # ref: https://gist.github.com/jordilin/08b6fed14b358dcbdf40c485c4daa730
          git branch -r --merged |
          grep origin |
          grep -v '>' |
          grep -v main |
          xargs -L1 |
          cut -d"/" -f2- |
          xargs git push origin --delete
        '';
        scripts = [
          gitcleanbranches
        ];
      in
      with pkgs;
      {
        devShell = mkShell {
          buildInputs = [
            openssl
            pkg-config
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" "llvm-tools-preview" ];
            })
            rust-analyzer
            cargo-tarpaulin
            cargo-audit
            cargo-watch
            # cargo-edit provides cargo upgrade for Cargo.toml
            # See note cargo upgrade stuck on fetch
            # https://github.com/killercup/cargo-edit/issues/869#issuecomment-1696223822
            # run CARGO_REGISTRIES_CRATES_IO_PROTOCOL=git cargo fetch
            # and CARGO_REGISTRIES_CRATES_IO_PROTOCOL=git cargo upgrade
            cargo-edit
            # cargo llvm-cov --html (alternative to tarpaulin needs
            # llvm-tools-preview extension)
            cargo-llvm-cov
            cargo-nextest
            sccache
            just
            python311Packages.requests
            python311Packages.black
            # Github actions locally for fast iteration
            act
            # Github actions linter
            actionlint
          ] ++ scripts;
          shellHook = ''
            rustc --version
            rustversion=$(rustc --version | awk '{print $2}')
            pythonversion=$(python --version | awk '{print $2}')
            SHELL_NAME="rc-$rustversion|py-$pythonversion"
            export PATH="./result/bin:$PATH"
          '';

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        };

       packages = {
        default = rustPlatform.buildRustPackage {
          pname = cargoManifest.name;
          version = cargoManifest.version;
          src = ./.;
          nativeBuildInputs = [ pkg-config openssl ];
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        };
      };
    }
  );
}
