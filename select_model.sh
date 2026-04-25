#!/usr/bin/env bash
# alias: `n/a`
# desc: flatten choice of model without worrying about vendor
# usage: fn_select_model.sh [args]

# set -x # uncomment to debug

__self_path_file=$(readlink -f "$0")
__self_path_dir=$(dirname "${__self_path_file}")

fn_select_model() {
  local model_name
  # Show "vendor/name" in fzf but return just the model name
  model_name=$(yq -r '.models[] | "\(.vendor)/\(.name)"' "${__self_path_dir}/config.yml" | fzf --with-nth=1) || exit 1

  # Extract just the name part (after the slash) for internal use
  local model_name_raw="${model_name##*/}"

  # query full data
  model_data=$(yq '.models[] | select(.name == "'"${model_name_raw}"'")' "${__self_path_dir}/config.yml")

  if [ -z "${model_data}" ]; then
    echo "Error: Model '${model_name_raw}' not found in config.yml" >&2
    exit 1
  fi

  echo '' >&2
  echo 'Selected:' >&2
  echo "$model_data" >&2
  echo '' >&2

  local vendor=$(echo "${model_data}" | yq '.vendor')

  echo "SESEPUH_HUB_MODEL=${model_name_raw}" >&2
  echo "SESEPUH_HUB_VENDOR=${vendor}" >&2

  export SESEPUH_HUB_MODEL="${model_name_raw}"
  export SESEPUH_HUB_VENDOR="${vendor}"

  if [[ "$*" == *"--consume-output"* ]]; then
    echo "SESEPUH_HUB_MODEL=${model_name_raw}" &&
    echo "SESEPUH_HUB_VENDOR=${vendor}"
    return $?
  fi
}

fn_select_model "$@"
