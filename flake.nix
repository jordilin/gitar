{
  description = "Gitar Nix integration";

  inputs = {
    nixpkgs.url      = "github:nixos/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url  = "github:numtide/flake-utils";
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, rust-overlay, crane, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        inherit (pkgs) lib;
        rust = (pkgs.rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" "llvm-tools-preview" ];
        });
        craneLib = (crane.mkLib pkgs).overrideToolchain rust;
        src = lib.cleanSourceWith {
          src = craneLib.path ./.;
        };
        commonArgs = {
          inherit src;
          strictDeps = true;
          nativeBuildInputs = [
            pkgs.pkg-config
          ];
          buildInputs = [
            pkgs.openssl
          ];
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        gr = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });

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
        releaseScript = builtins.readFile ./release.sh;
        release = pkgs.writeShellScriptBin "release" releaseScript;

        scripts = [
          gitcleanbranches
          release
        ];
      in
      with pkgs;
      {
        checks = {
          inherit gr;
        };

        packages = {
          default = gr;
          inherit gr;
        };

        devShells.default = craneLib.devShell {
          inputsFrom = [ gr ];
          checks = self.checks.${system};
          packages = [
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
    }
  );
}
