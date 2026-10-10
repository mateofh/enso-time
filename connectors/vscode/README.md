# Ensō Time Context para VS Code/Cursor

1. Empaqueta e instala la extensión (requiere Node.js 18+):

   ```sh
   npx @vscode/vsce package --allow-missing-repository --skip-license
   code --install-extension enso-time-context-0.1.0.vsix    # o: cursor --install-extension …
   ```

2. En Ensō Time activa **Editor**, pulsa **Emparejar** y copia el código.
3. Ejecuta `Ensō Time: Emparejar con la aplicación` desde la paleta.

La extensión envía workspace, repositorio, rama y tipo de archivo una vez por minuto o al cambiar de editor. No lee código automáticamente. El único comando que comparte contenido es `Ensō Time: Compartir selección una vez`, con confirmación modal.

Guía completa y solución de problemas: [`docs/CONNECTORS.md`](../../docs/CONNECTORS.md).
