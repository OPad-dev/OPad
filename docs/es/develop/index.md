---
title: Guía para desarrolladores
description: Flujo de trabajo para colaboradores, invariantes de producto, ciclo de vida de ramas git, convenciones de commits y reglas de documentación de OPad.
sidebar:
  order: 1
---

Bienvenido a la documentación para desarrolladores de OPad. Esta sección cubre las reglas para colaboradores, la compilación desde el código fuente, la ejecución de pruebas automatizadas, los detalles internos del firmware y del escritorio, la evolución del protocolo, los archivos de hardware, el empaquetado, los lanzamientos, las pruebas de laboratorio y las mediciones de latencia.

## Invariantes principales de producto

Todo cambio en este repositorio debe respetar las seis reglas de producto en [/AGENTS.md](/AGENTS.md) §1:

1. **La latencia siempre gana**: Si una funcionalidad empeora de forma medible la latencia de tecla o su fluctuación (jitter), se reduce, se difiere, se congela durante la partida o se elimina. Nada entra en la ruta de pulsación: la pantalla, el almacenamiento, la bandeja del sistema y `tosu` permanecen fuera.
2. **El teclado es un teclado sin software instalado**: Teclado USB HID a 1000 Hz que no requiere controladores ni software de escritorio en Linux o Windows 10/11.
3. **La app y el teclado se actualizan de forma independiente**: Los cambios de protocolo y formato en el cable deben funcionar en ambos sentidos (app nueva con firmware antiguo, app antigua con firmware nuevo) mediante detección y negociación.
4. **La pantalla táctil es el tercer botón**: Pulsación/mantenimiento/liberación limpios; Quick Retry (`` ` ``) durante un mapa (nunca desde deslizamientos); deslizamientos configurables fuera de un mapa; nunca inserta modificadores ni teclas extra en los reportes de teclado durante un mapa; excepción: un teclado Hall Effect sin calibrar queda bloqueado en su pantalla de calibración (D7).
5. **Nunca los botones BOOT o RESET**: El flasheo, la actualización y la recuperación ocurren por USB/serie con reversión OTA A/B y `opadctl recover`.
6. **Cada tecla proviene de un dedo**: Sin macros, auto-tap, entrada programada/temporizada ni comandos anfitrión-a-teclado que inyecten teclas.

## Flujo de trabajo Git y reglas de commits

Según [/AGENTS.md](/AGENTS.md) §2–§3:

- **Historial lineal**: Rebase sobre `main`; sin commits de mezcla (merge commits).
- **Ramas de corta duración**: Los mantenedores trabajan en un worktree en su propia rama, hacen merge fast-forward a `main` y luego eliminan la rama y el worktree. Los colaboradores externos hacen fork, crean una rama y abren un PR contra `main`.
- **Autoría de commits y divulgación de IA**: Firma siempre los commits con tu propio nombre y correo (nunca con una IA como autor). Termina los commits asistidos por IA con un solo pie (`AI: assisted` o `AI: generated`); omítelo si no se usó IA. Reemplaza líneas específicas de herramientas como `Co-Authored-By` por este pie.
- **Commits convencionales**: `type(scope): plain statement of behaviour (#issue)`. Añade un pie `Compatibility:` siempre que cambie el comportamiento del protocolo o de IPC.
- **Licencias (REUSE)**: Los archivos nuevos necesitan cabeceras de licencia o una entrada en [/REUSE.toml](/REUSE.toml).

## El código como única fuente de verdad

Según [/AGENTS.md](/AGENTS.md) §7:

- Cuando la documentación y el código discrepen, gana el código: corrige o elimina el documento obsoleto.
- Las páginas de desarrollo (`docs/explanation/*`, `docs/develop/*`) enlazan archivos fuente mediante rutas desde la raíz del repositorio (por ejemplo, `[/firmware/main/app_main.c](/firmware/main/app_main.c)`).
- Nunca inventes opciones de CLI, campos de configuración ni pines GPIO que no existan en el código base.

## Estructura de la documentación (Diátaxis)

- **Inicio (`docs/start/*`)**: Tutoriales de primeros pasos (hardware, cableado, primera conexión, instalación de la app).
- **Guías (`docs/guides/*`, `docs/recovery.md`)**: Procedimientos paso a paso orientados a tareas.
- **Referencia (`docs/reference/*`, `docs/troubleshooting.md`, `docs/develop/*`)**: Catálogos autorizados de CLI, GUI, ajustes, pinout, protocolos serie/IPC, rutas e implementación interna.
- **Explicaciones (`docs/explanation/*`)**: Arquitectura del sistema y justificación de diseño.
