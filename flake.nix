{
  description = "Decision Engine: dev shell and container-free local infrastructure";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      # `nix run .#infra` — the services `oneclick.sh` would otherwise start with `docker compose`
      # (Postgres, Redis, Kafka, ClickHouse, Mailpit, OpenTelemetry collector), run as plain
      # processes under process-compose. For machines without Docker (e.g. NixOS dev boxes).
      # Run it from the repo root; state lives in `.nix-infra/` (override with DE_INFRA_DIR).
      # Extra arguments go to `process-compose up`, e.g. `nix run .#infra -- -t=false`.
      packages = forAllSystems (pkgs: {
        infra = pkgs.writeShellApplication {
          name = "decision-engine-infra";
          runtimeInputs = with pkgs; [
            process-compose
            postgresql_16
            redis
            apacheKafka
            clickhouse
            mailpit
            opentelemetry-collector-contrib
          ];
          text = builtins.readFile ./nix/infra.sh;
        };
        default = self.packages.${pkgs.system}.infra;
      });

      apps = forAllSystems (pkgs: {
        infra = {
          type = "app";
          program = "${self.packages.${pkgs.system}.infra}/bin/decision-engine-infra";
        };
      });

      # `nix develop` — everything `oneclick.sh` and the cargo builds need.
      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            pkg-config
            cmake
            clang
            protobuf
            just
            (diesel-cli.override { sqliteSupport = false; mysqlSupport = false; })
            nodejs_22
            postgresql_16
            redis
            lsof
            curl
            jq
          ];
          buildInputs = with pkgs; [ openssl libmysqlclient ];

          # bindgen (mysqlclient-sys) needs libclang.
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          # jemalloc-sys 0.5 does not build as C23, the default of current GCC. Its configure
          # probes also need optimization on, or _FORTIFY_SOURCE's warning fails the strerror_r
          # check and the build breaks.
          CFLAGS = "-std=gnu17 -O1";

          # Global npm installs (the `mint` docs preview oneclick.sh starts) go to the repo, not
          # a root-owned prefix: run `npm install -g mint` once inside the shell.
          shellHook = ''
            export NPM_CONFIG_PREFIX="$PWD/.nix-infra/npm"
            export PATH="$NPM_CONFIG_PREFIX/bin:$PATH"
          '';
        };
      });
    };
}
