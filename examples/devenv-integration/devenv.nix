{ pkgs, ... }:

{
  # Development packages
  packages = with pkgs; [ nodejs_20 yarn git ];

  # Environment variables from devenv
  env = {
    NODE_ENV = "development";
  };

  # Services
  services.postgres = {
    enable = true;
    package = pkgs.postgresql_15;
    listen_addresses = "localhost";
    port = 5432;
  };

  # Scripts
  scripts.hello.exec = "echo 'Hello from devenv!'";
}
