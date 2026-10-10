# Ensō Time Context para terminal

Empareja una vez:

```sh
python3 enso-context.py --pair 12345678
```

Después carga `enso-context.zsh` o `enso-context.bash`. En PowerShell, carga `EnsoTimeContext.ps1` e invoca `Invoke-EnsoTimeContext` desde tu función `prompt`.

El conector reporta como máximo una vez por minuto: directorio actual, raíz Git y rama. No lee ni almacena el comando, el historial, el contenido del repositorio ni variables de entorno.

Guía completa y solución de problemas: [`docs/CONNECTORS.md`](../../docs/CONNECTORS.md).
