# Ensō Time Context para Chromium

1. Abre `chrome://extensions` (o `edge://extensions`) y activa **Modo desarrollador**.
2. Pulsa **Cargar descomprimida** y elige esta carpeta.
3. En Ensō Time activa **Navegador**, guarda la configuración y pulsa **Emparejar**.
4. Abre las opciones de la extensión e introduce el código de ocho dígitos.

Por defecto se envían una vez por minuto únicamente el título y dominio de la pestaña
activa. Las pestañas de incógnito y las URL no HTTP(S) se ignoran. Para compartir texto
visible de un sitio, añádelo tanto a la lista de la extensión como a la lista autorizada
de Ensō Time; Chrome solicitará el permiso de sitio correspondiente.

El token se guarda en `chrome.storage.local`, funciona solo contra el servidor loopback
de Ensō Time y puede revocarse desde la aplicación. Desactivar Navegador en Ensō Time
hace que el servidor rechace inmediatamente nuevos eventos.
