{ pkgs, ... }:

{
  packages = with pkgs; [ nodejs_20 ];

  env = {
    NODE_VERSION = "20";
  };

  scripts.hello.exec = "echo 'Hello from native Devenv integration!'";
}
