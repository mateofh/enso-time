# Source this file from .zshrc after pairing enso-context.py.
typeset -g ENSO_TIME_CONTEXT_SCRIPT="${${(%):-%N}:A:h}/enso-context.py"
function _enso_time_context_precmd() {
  command python3 "$ENSO_TIME_CONTEXT_SCRIPT" --send >/dev/null 2>&1 &!
}
autoload -Uz add-zsh-hook
add-zsh-hook precmd _enso_time_context_precmd
