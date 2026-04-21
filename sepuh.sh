#!/usr/bin/env bash
# alias: '::'
# desc: fn_sepuh_hub description.
# usage: fn_sepuh_hub.sh [args]

set -e -u -o pipefail
# set -x # uncomment to debug

declare -r __self_path_file=$(readlink -f "$0")
declare -r __self_path_dir=$(dirname "${__self_path_file}")

# check if script run directly or indirect
# if [ "${0}" = "${BASH_SOURCE}" ]; then
#   echo "Script is being run directly"
# else
#   echo "Script is being sourced"
# fi

fn_sepuh() {
  "${__self_path_dir}/target/release/sepuh" --prompt "$*"
}

fn_sepuh "$*"
