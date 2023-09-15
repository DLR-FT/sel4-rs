{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-22.11";
    utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  # nixConfig = {
  #   allow-import-from-derivation = true;
  # };

  outputs = { self, nixpkgs, utils, fenix, treefmt-nix, naersk, ... }@inputs:
    utils.lib.eachSystem [
      #"aarch64-linux" "i686-linux"
      "x86_64-linux"
    ]
      (system:
        let
          lib = nixpkgs.lib;
          pkgs = import nixpkgs {
            inherit system;
          };

          treefmt = {
            projectRootFile = "flake.nix";
            settings = with builtins; (fromTOML (readFile ./treefmt.toml));
          };

          # generates an attr set with stable, beta and nightly as attrs. Each of them
          # is a drv, that contains a full rust toolchain, including all targets described
          # in Platforms.toml
          rust-toolchains =
            let
              fenix' = fenix.packages.${system};
              # unfortunately fenix doesn't name the nightly toolchain 'nightly'
              releaseChannels = {
                stable = "stable";
                beta = "beta";
                nightly = "latest";
              };
            in
            lib.mapAttrs
              (name: value: fenix'.combine (
                with fenix'.${value}; [
                  rustc
                  cargo
                  clippy
                  rustfmt
                  rust-src
                  rust-analyzer
                  fenix'.targets.armv7a-none-eabi.latest.rust-std
                ]
              ))
              releaseChannels;

          # overrides a naersk-lib which uses the stable toolchain expressed above
          naersk-lib = (naersk.lib.${system}.override {
            cargo = rust-toolchains.stable;
            rustc = rust-toolchains.stable;
          });
        in
        rec {

          packages = {
            #
            ### seL4 Kernels
            #

            seL4-arm =
              let
                # pkgs set for the specific target
                pkgsTarget = pkgs.pkgsCross.armhf-embedded;

                # evaluates to arm-none-eabihf-
                inherit (pkgsTarget.stdenv.cc) targetPrefix;
              in
              pkgs.stdenvNoCC.mkDerivation {
                name = "seL4";
                version = "unknown";
                src = ./sel4-sys/sel4; #seL4-kernel-src;
                nativeBuildInputs = with pkgs; [
                  pkgsTarget.stdenv.cc # arm-linux-gnueabi-gcc
                  cmake
                  ninja
                  dtc
                  cpio
                  (python3.withPackages (ps: with ps; [
                    ply
                    pip
                    jsonschema
                    pyyaml
                    jinja2
                    pyelftools
                    libarchive-c
                    packages.pyfdt
                    setuptools
                    future
                  ]))
                  libxml2 #xmllint
                ];

                preConfigure = ''
                  patchShebangs kernel/tools
                  mkdir $out
                '';

                cmakeFlags = [
                  "-GNinja"
                  "-DCMAKE_TOOLCHAIN_FILE=kernel/gcc.cmake"
                  "-DPLATFORM=zynq7000"
                  "-DRELEASE=FALSE"
                  "-DVERIFICATION=FALSE"
                  "-DKernelIsMCS=ON"
                  "-DLibSel4FunctionAttributes=public"
                  "-DKernelDangerousCodeInjection=ON"
                  "-DCMAKE_EXPORT_COMPILE_COMMANDS=1"
                  "-DCROSS_COMPILER_PREFIX=${targetPrefix}"
                  "-B${placeholder "out"}"
                ];

                postConfigure = ''
                  cd $out
                '';

                installPhase = ":";
                dontFixup = true;
              };


            #
            ### External Tools
            #
            lcov-cobertura = with pkgs.python3Packages;
              buildPythonPackage
                rec {
                  pname = "lcov_cobertura";
                  version = "2.0.1";
                  src = fetchPypi {
                    inherit pname version;
                    sha256 = "sha256-4iEgE5IZU3BULu8o4GiYplSMzCG3KR4bf41YAddQyA8=";
                  };
                };
            pyfdt = with pkgs.python3Packages; buildPythonPackage
              rec {
                pname = "pyfdt";
                version = "0.3";
                src = fetchPypi {
                  inherit pname version;
                  sha256 = "sha256-YWAcIAX/OUolpshMbaIIi7+IgygDhADSfk7rGwS59PA=";
                };
              };
          };


          devShells = rec {
            default = pkgs.mkShellNoCC {

              inputsFrom = [ packages.seL4-arm ];

              nativeBuildInputs = with pkgs;
                [
                  # rust stuff
                  rust-toolchains.nightly

                  # seL4 stuff
                  gdb
                  qemu_full

                  # tools
                  pkgs.rustPlatform.bindgenHook # our rust toolchain
                  pkgs.gitlab-clippy # convert clippy reports to GitLab Code Quality Reports
                  pkgs.cargo-nextest # run cargo test and generate JUnit reports
                  pkgs.cargo-llvm-cov # run cargo test and generate coverage reports
                  pkgs.treefmt # formatting orchestrator
                  pkgs.nixpkgs-fmt # formatting nix files
                  pkgs.nodePackages.prettier # prettifier for MarkDown and YAML
                ];
            };
          };

          formatter = treefmt-nix.lib.mkWrapper
            pkgs
            treefmt;

          checks = {
            treefmt = ((treefmt-nix.lib.evalModule pkgs treefmt).config.build.check self).overrideAttrs (o: {
              buildInputs = o.buildInputs ++ devShells.default.nativeBuildInputs;
            });
          };

          # tell the CI server what to build
          hydraJobs = checks;
        });
}




