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
        source = builtins.readFile ./tests/fail_msg_to_stderr.sh;
        test_shell_fail_script = pkgs.writeScriptBin "fail_msg_to_stderr.sh" source;
      in
      with pkgs;
      {


        devShell = mkShell {
          buildInputs = [
            openssl
            pkgconfig
            (rust-bin.stable.latest.default.override {
              extensions = [ "rust-src" "llvm-tools-preview" ];
            })
            rust-analyzer
            cargo-tarpaulin
            cargo-audit
            cargo-watch
            # cargo llvm-cov --html (alternative to tarpaulin needs
            # llvm-tools-preview extension)
            cargo-llvm-cov
            cargo-nextest
            sccache
            just
            python311Packages.requests
            python311Packages.black
          ];
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
          nativeBuildInputs = [ pkgconfig openssl ];
          cargoLock = {
            lockFile = ./Cargo.lock;
          };
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
        };
      };
    }
  );
}
