{
  description = "Rust BSP layout component and paired GAMS UI Service";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/d233902339c02a9c334e7e593de68855ad26c4cb";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };
  outputs = {
    nixpkgs,
    rust-overlay,
    ...
  }: let
    systems = ["aarch64-darwin" "x86_64-darwin" "aarch64-linux" "x86_64-linux"];
    each = nixpkgs.lib.genAttrs systems;
  in {
    devShells = each (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [rust-overlay.overlays.default];
      };
      rust = pkgs.rust-bin.stable."1.94.1".default.override {targets = ["wasm32-wasip2"];};
    in {
      default = pkgs.mkShell {
        packages = [rust pkgs.gnumake pkgs.python3 pkgs.nodejs_24 pkgs.curl pkgs.unzip pkgs.zip pkgs.wasm-tools];
      };
    });
  };
}
