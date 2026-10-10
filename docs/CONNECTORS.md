# Installing the context connectors

The connectors are optional. They add browser, editor, and terminal context to the
timeline so the Automation Blueprint has more evidence to work with. Everything stays on
this computer: each connector talks only to Ensō Time on `127.0.0.1`.

Connectors are not in the installers yet. Download **Source code (zip)** from the
[release](https://github.com/mateofh/enso-time/releases) that matches your Ensō Time
version, or clone the repository, and keep the `connectors/` folder somewhere permanent.
Chrome loads the extension from that path, so moving or deleting the folder removes it.

## Before you pair

1. Ensō Time is open and tracking is **running**. Connectors are rejected while tracking
   is paused or outside your work schedule.
2. In **Local context connectors**, turn on the source you want (**Active browser tab**,
   **VS Code / Cursor project**, or **Terminal project**) and save.
3. Press **Pair** next to that source. Ensō Time shows an **eight-digit code** that is
   valid for **five minutes** and works once.

## Chromium (Chrome, Edge, Brave, Arc)

1. Open `chrome://extensions` (`edge://extensions` in Edge) and turn on **Developer mode**.
2. Press **Load unpacked** and select `connectors/chromium`.
3. Open the extension's **Options**, enter the code, and save.

Do not edit `manifest.json`. Its `key` pins the extension ID, and Ensō Time only accepts
requests from that ID.

By default the extension sends the active tab's title and domain once a minute. Incognito
tabs and non-HTTP(S) pages are ignored. To share visible page text for a site, add it to
both the extension's list and the Ensō Time allowlist; Chrome then asks for that site's
permission.

**Updating:** replace the folder with the new version and press the reload icon on the
extension card.

## VS Code and Cursor

Requires Node.js 18 or newer to package the extension once.

```sh
cd connectors/vscode
npx @vscode/vsce package --allow-missing-repository --skip-license
code --install-extension enso-time-context-0.1.0.vsix     # VS Code
cursor --install-extension enso-time-context-0.1.0.vsix   # Cursor
```

If the `code` or `cursor` command is missing, use **Extensions → … → Install from
VSIX…** in the editor instead.

Then open the command palette, run **Ensō Time: Emparejar con la aplicación**, and paste
the code.

The extension sends workspace, repository, branch, and file type once a minute or when you
switch editors. It never reads code on its own. **Ensō Time: Compartir selección una vez**
shares the current selection after a confirmation dialog, and **Ensō Time: Desconectar
este editor** forgets the token.

## Terminal

Requires Python 3.8 or newer. Pair once with the code:

```sh
python3 /path/to/connectors/shell/enso-context.py --pair 12345678
```

Then load the hook for your shell:

| Shell | Add to | Line |
|---|---|---|
| zsh | `~/.zshrc` | `source /path/to/connectors/shell/enso-context.zsh` |
| bash | `~/.bashrc` | `source /path/to/connectors/shell/enso-context.bash` |
| PowerShell | `$PROFILE` | `. C:\path\to\connectors\shell\EnsoTimeContext.ps1`, then call `Invoke-EnsoTimeContext` inside your `prompt` function |

Open a new terminal. The hook reports at most once a minute: current directory, Git root,
and branch. It never reads commands, history, file contents, or environment variables.
The token is stored in `shell.json` under Ensō Time's connector folder in your user
application data.

## Troubleshooting

| Message or symptom | Cause | Fix |
|---|---|---|
| "No se encontró Ensō Time" or no server found | The app is closed | Open Ensō Time and try again |
| Code rejected | Wrong, already used, or older than five minutes | Press **Pair** again for a fresh code |
| "Demasiados intentos" (HTTP 429) | Five wrong codes in a row | Wait one minute; pending codes were discarded, so request a new one |
| Nothing appears while paused (HTTP 423) | Tracking is paused or outside the work schedule | Expected. Resume tracking |
| "Esta fuente fue deshabilitada" | The source is off in Settings | Turn it back on and save |
| "Token inválido o revocado" (HTTP 401) | You revoked the connector or reinstalled Ensō Time | Pair again |

Revoking a connector in Settings, or turning off its source, takes effect immediately even
if the connector still holds its token. Context follows the same retention period as the
timeline.
