# Source this file from .bashrc after pairing enso-context.py.
ENSO_TIME_CONTEXT_SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/enso-context.py"
_enso_time_context_prompt() {
  command python3 "$ENSO_TIME_CONTEXT_SCRIPT" --send >/dev/null 2>&1 &
}
PROMPT_COMMAND="_enso_time_context_prompt${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
