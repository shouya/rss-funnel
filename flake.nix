{
  description = "Build a cargo project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, crane, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        inherit (pkgs) lib stdenv;

        craneLib = crane.mkLib pkgs;
        src = lib.fileset.toSource {
          root = ./.;
          fileset = lib.fileset.unions [
            (craneLib.fileset.commonCargoSources ./.)
            ./static # Include front-end assets
            ./fixtures # Include fixtures for testing
          ];
        };

        pnpmPkg = pkgs.pnpm_9;
        inspector-ui = stdenv.mkDerivation (final: {
          pname = "rss-funnel-inspector-ui";
          version = "0.0.0";
          src = lib.fileset.toSource {
            root = ./inspector;
            fileset = lib.fileset.unions [
              ./inspector/src
              ./inspector/package.json
              ./inspector/pnpm-lock.yaml
            ];
          };
          nativeBuildInputs = [ pkgs.nodejs pnpmPkg.configHook ];
          pnpmDeps = pnpmPkg.fetchDeps {
            inherit (final) pname version src;
            hash = "sha256-puyd8AbeMBsTw/Ua5yQMATI0bwum4hnK5advXI2Y10k=";
          };

          buildPhase = ''
            runHook preBuild
            pnpm build
            runHook postBuild
          '';
          installPhase = ''
            runHook preInstall
            mkdir -p $out/lib
            cp -r dist/* $out/lib/
            runHook postInstall
          '';
        });

        depsArgs = {
          inherit src;
          strictDeps = true;
          nativeBuildInputs = [pkgs.pkg-config];
        };
        buildArgs = depsArgs // {
          preBuild = ''
            mkdir -p inspector/dist/
            cp -r ${inspector-ui}/lib/* inspector/dist/
          '';
          nativeBuildInputs = [inspector-ui];
          cargoArtifacts = craneLib.buildDepsOnly depsArgs;
          cargoTestCommand = "cargo test --profile release --features _test-offline";
        };

        rss-funnel = craneLib.buildPackage buildArgs;
      in {
        checks = {inherit rss-funnel;};
        packages.default = rss-funnel;
        apps.default = flake-utils.lib.mkApp {drv = rss-funnel;};

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};
          packages = with pkgs; [
            rust-analyzer
            pnpmPkg
          ];
        };
      });
}
