{ pkgs, lib, config, inputs, ... }:

{
  # Directory used to output final build artifacts
  env.ARTIFACTS_DIR = "${config.env.DEVENV_ROOT}/artifacts";
  env.CGO_ENABLED = 1;

  packages = [
    pkgs.git
    pkgs.maturin # Used to build Python wheels
    pkgs.unzip
  ];
  languages.rust = {
    enable = true;
  };
  languages.go = {
    enable = true;
  };
  languages.python = {
    enable = true;
    venv.enable = true;
  };

  scripts.build-driver.exec = ''
    cargo_configuration="dev"
    target_dir="$DEVENV_ROOT/target/debug"
    if [ -n "$CONFIGURATION" ]; then
      cargo_configuration="$CONFIGURATION"
      target_dir="$DEVENV_ROOT/target/$CONFIGURATION"
    fi

    echo "**** Building C API ****"
    cargo build --features c_api --profile $cargo_configuration

    if [ ! -d "$ARTIFACTS_DIR/lib" ]; then
      mkdir -p $ARTIFACTS_DIR/lib
    fi

    cp $target_dir/libazure_data_cosmos_client_engine.so $ARTIFACTS_DIR/lib/libcosmoscx.so
    cp $target_dir/libazure_data_cosmos_client_engine.a $ARTIFACTS_DIR/lib/libcosmoscx.a

    echo "**** Building Python extension module ****"
    cd "$DEVENV_ROOT/python"
    maturin develop
  '';

  enterShell = ''
    export CC="${pkgs.clang}/bin/clang"
  '';
}
