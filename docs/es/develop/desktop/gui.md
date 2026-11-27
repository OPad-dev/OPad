---
title: Arquitectura del subsistema de la GUI
description: Referencia de implementación de opad-gui, su bucle de actualización iced y el flujo del daemon, el arranque del daemon y la comprobación de versión, el traspaso a la instancia única, los tokens del tema, la vista previa del Screen Designer y la bandeja del sistema.
sidebar:
  order: 3
---

`opad-gui` es una app de `iced` 0.14 construida con `iced::daemon`, así que al cerrar la ventana se queda en la bandeja del sistema. No guarda estado propio del pad: todo le llega de `opad-daemon` por IPC ([/desktop/gui/src/main.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/main.rs), [/desktop/gui/src/ipc.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/ipc.rs)). Qué hace cada página para el usuario: [Referencia de la GUI](../../reference/gui.md).

## Módulos

| Módulo | Función |
|---|---|
| [`main.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/main.rs) | `App` (`new`, `update`, `view`, `subscription`); `Page`, `Section` y `Target` con los tokens de `--page`; el único modelo de guardado de los ajustes del pad (`form_config`, `pad_dirty`); los avisos; arranque del daemon y coincidencia de versiones |
| [`pages.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/pages.rs) | Dashboard, Pad, la pestaña Logs y la vista de calibración |
| [`keys.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/keys.rs) | Página Keys: asignación de teclas, cableado, y actuación HE y Rapid Trigger con un corte en vivo de cada interruptor |
| [`screen.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/screen.rs) | Página Screen: pantalla, gestos de la pantalla táctil, animaciones de resultado |
| [`settings.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/settings.rs) | Página Settings y sus pestañas: General, Updates, osu! connection, About |
| [`stats.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/stats.rs) | Página Stats: velocidad de tecla, y frecuencia de toques en KPM del intento y del periodo del historial |
| [`troubleshooting.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/troubleshooting.rs), [`he_monitor.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/he_monitor.rs) | Página Troubleshooting: prueba de teclas, prueba de placa (evaluada por `opad_model::board_test`), conexión, informe, y el monitor de pulsaciones fantasma / dobles HE que sigue `he-debug.log` |
| [`widgets.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/widgets.rs) | Lo que comparten las páginas: tarjetas, secciones, la barra de pestañas y la barra de guardado |
| [`designer/`](https://github.com/OPad-dev/OPad/tree/main/desktop/gui/src/designer) | Screen Designer (`mod.rs`, `overlay.rs`) |
| [`chrome.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/chrome.rs) | El marco propio de la ventana en lugar de las decoraciones del sistema: esquinas redondeadas, el logo y los botones de ventana (ambos arrastran la ventana), bordes de redimensionado |
| [`theme.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/theme.rs) | Colores y fuentes |
| [`feed.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/feed.rs) | La conexión por la que el daemon envía sus respuestas |
| [`tray.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/tray.rs) | Icono y menú de la bandeja |
| [`single_instance.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/single_instance.rs) | Una sola GUI a la vez |
| [`platform_linux.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/platform_linux.rs), [`platform_windows.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/platform_windows.rs) | Arranque automático (unidades de usuario de systemd en Linux, el mecanismo por usuario en Windows) y otras piezas de integración con el sistema operativo |

## Bucle de actualización y el flujo

La GUI mantiene abierta una conexión con el daemon y se suscribe a lo que muestra la página abierta (`App::watches`, [`feed.rs`](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/feed.rs), [suscripciones IPC](../../reference/ipc.md#suscripciones)). El daemon envía cada respuesta cuando cambia, y `App::watched` la convierte en el mensaje en que se convierte la misma respuesta a una petición:

| Vigilado | Cuándo | El daemon mira cada |
|---|---|---|
| `GetStatus` | Siempre | 100 ms; 1 s con la ventana oculta en la bandeja |
| `GetUpdateStatus` | Cualquier página | 500 ms |
| `GetUiValues` | Dashboard, Designer | 250 ms |
| `GetFirmwareUpdate` | Settings, Pad, Troubleshooting; cualquier página mientras el pad es incompatible | 1 s |
| `GetTapStats` | Stats | 500 ms |
| `GetLogEntries` | Troubleshooting → Logs | 250 ms |
| `GetHeLive` | Keys, fuera de la calibración | 20 ms; los vástagos se redibujan en los fotogramas de animación |

«Traer ahora» (`App::poll`, tras una acción o al abrir una página) hace una suscripción nueva, por la que el daemon envía de nuevo todas las respuestas; una petición enviada al margen del flujo podría llegar después de una respuesta más reciente.

Los temporizadores son el respaldo (`FeedState`): preguntan como dice la tabla siguiente hasta que la suscripción está activa, de nuevo cuando su conexión termina, y siempre con un daemon que no conoce `Subscribe`. Un flujo perdido se vuelve a intentar cuando el daemon responde a una petición. Cuando un flujo activo termina, el estado se pide en el acto, así que un daemon que ha desaparecido se muestra como desconectado; la app lo vuelve a arrancar solo en el siguiente estado del temporizador, unos 2 s después (`daemon_start_held`), porque un instalador o el desinstalador que ha parado el daemon para la app a continuación, y un daemon arrancado entre medias sobreviviría a ambos.

| Temporizador | Intervalo |
|---|---|
| Estado (`Message::Poll`) | 2 s con el daemon desconectado, 1 s con la ventana abierta, 3 s oculta en la bandeja |
| Muestras HE en vivo (página Keys) | 20 ms |
| Estado de las actualizaciones | 60 s, 2 s mientras hay una actualización en curso |
| Progreso de la calibración (la calibración dentro de Keys) | 100 ms; este también funciona con el flujo |

Si el daemon no está en marcha, la GUI lo arranca una vez. Si el daemon responde con otra versión (se actualizó uno de los dos), el más antiguo se reinicia como la versión nueva, una vez por ejecución.

## Flags de CLI e instancia única

- `--tray` arranca oculta en la bandeja; `--page <token>` / `-p <token>` abre una página o una de sus pestañas. Los tokens son `Target::TOKENS`; los nombres que tenían antes las páginas siguen abriendo el sitio al que fue su contenido (`device` abre Pad, `monitor` la pestaña Logs, `diagnostics` Troubleshooting). La lista completa: [Referencia de la GUI](../../reference/gui.md#páginas).
- Un segundo lanzamiento pide a la instancia en ejecución que muestre su ventana (y abra su `--page`) y después termina. En Unix, la instancia hace bind de `gui.sock` junto al socket de IPC; de dos lanzamientos simultáneos, exactamente uno gana el bind y el otro se conecta. Un socket sobrante solo se borra cuando conectarse a él falla con `ECONNREFUSED`. En Windows decide un mutex con nombre y la petición va por el pipe `\\.\pipe\opad-gui`.

## Tema y tipografía (`theme.rs`)

La paleta es la misma que la del pad ([/firmware/main/ui/core/ui_defaults.c](https://github.com/OPad-dev/OPad/blob/main/firmware/main/ui/core/ui_defaults.c)): `BG = #0E0E16`, `SURFACE = #13131E`, `CARD = #1B1B28`, `CARD_HOVER = #24243A`, `BORDER = #2A2A3C`, `PINK = #FF66AA`, `WHITE = #FFFFFF`, `MUTED = #8C8CA6`, `GREEN = #44DD88`, `RED = #FF5566`, `CYAN = #66CCFF`, `YELLOW = #FFCC33`, más `PINK_HOVER` y `OFF`. Incrusta `Montserrat-Medium.ttf` y `Montserrat-Bold.ttf` (SIL OFL 1.1).

## Screen Designer (`designer/`)

[/desktop/gui/src/designer/mod.rs](https://github.com/OPad-dev/OPad/blob/main/desktop/gui/src/designer/mod.rs) edita los layouts Idle y Playing (hasta 32 widgets, validados por `opad-layout`) en un lienzo a `SCALE = 2.0` con tiradores de redimensionado de `HANDLE = 5` píxeles del pad (`overlay.rs`). La imagen de fondo la renderiza `opad-ui-preview`, el propio código `ui/core` del firmware compilado para el host ([crates](crates.md#7-opad-ui-preview)). Guía de usuario: [Personalizar el layout de la pantalla](../../guides/customize-screen-layout.md).

## Bandeja del sistema (`tray.rs`)

Un modelo de vista independiente del backend (`IconState`: `Warning`, `Connected`, `Disconnected`; `TrayAction`: `ShowWindow`, `Open(Page)`, `SyncNow`, `StartDaemon`, `Quit`) implementado con `ksni` (StatusNotifierItem) en Linux y `tray-icon` en Windows.
