{ pkgs, lib, config, inputs, ... }:

{
  # Directory used to output final build artifacts
  env.ARTIFACTS_DIR = "${config.env.DEVENV_ROOT}/artifacts";

  packages = [
    pkgs.git
  ];
  languages.rust = {
    enable = true;
  };
  languages.go = {
    enable = true;
  };
}
