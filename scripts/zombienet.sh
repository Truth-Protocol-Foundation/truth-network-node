#!/bin/bash

ZOMBIENET_V=v1.3.91
POLKADOT_V=v1.0.0

case "$(uname -s)" in
    Linux*)     MACHINE=Linux;;
    Darwin*)    MACHINE=Mac;;
    *)          exit 1
esac

if [ $MACHINE = "Linux" ]; then
  ZOMBIENET_BIN=zombienet-linux-x64
elif [ $MACHINE = "Mac" ]; then
  ZOMBIENET_BIN=zombienet-macos
fi

BIN_DIR=bin-$POLKADOT_V

zombienet_init() {
  if [ ! -f $ZOMBIENET_BIN ]; then
    echo "fetching zombienet executable..."
    curl -LO https://github.com/paritytech/zombienet/releases/download/$ZOMBIENET_V/$ZOMBIENET_BIN
    chmod +x $ZOMBIENET_BIN
  fi
}

zombienet_build() {
  if [ ! -f $ZOMBIENET_BIN ]; then
    echo "fetching zombienet executable..."
    curl -LO https://github.com/paritytech/zombienet/releases/download/$ZOMBIENET_V/$ZOMBIENET_BIN
    chmod +x $ZOMBIENET_BIN
  fi
}

zombienet_devnet() {
  zombienet_init
  cargo build --release
  echo "spawning local chain nodes."
  local dir=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
  ./$ZOMBIENET_BIN spawn "$dir/../zombienet-config.toml" -p native
}


print_help() {
  echo "This is a shell script to automate the execution of zombienet."
  echo ""
  echo "$ ./zombienet.sh init         # fetches zombienet and polkadot executables"
  echo "$ ./zombienet.sh build        # builds polkadot executables from source"
  echo "$ ./zombienet.sh devnet       # spawns a local dev chain"
}

SUBCOMMAND=$1
case $SUBCOMMAND in
  "" | "-h" | "--help")
    print_help
    ;;
  *)
    shift
    zombienet_${SUBCOMMAND} $@
    if [ $? = 127 ]; then
      echo "Error: '$SUBCOMMAND' is not a known SUBCOMMAND." >&2
      echo "Run './zombienet.sh --help' for a list of known subcommands." >&2
        exit 1
    fi
  ;;
esac
