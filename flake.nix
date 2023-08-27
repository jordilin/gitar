{
  description = "A devShell example";

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
          '';

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
        };

      }
    );
}
