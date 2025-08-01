{ pkgs, ... }:

{
  # Development packages
  packages = with pkgs; [ nodejs_20 yarn python3 ];

  # Environment variables from devenv
  env = {
    NODE_ENV = "development";
    DEVENV_ACTIVE = "true";
  };

  # Scripts
  scripts.hello.exec = "echo 'Hello from devenv!'";
}
