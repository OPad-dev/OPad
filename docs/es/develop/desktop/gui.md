---
title: Arquitectura del subsistema GUI
description: Referencia de implementación de opad-gui, arquitectura Elm de iced, traspaso de instancia única, tokens de tema, vista previa de Screen Designer y bandeja del sistema.
sidebar:
  order: 3
---

`opad-gui` está construida con `iced` usando `iced::daemon` (de modo que cerrar la ventana minimiza a la bandeja del sistema) y se comunica asíncronamente con `opad-daemon` mediante IPC local ([/desktop/gui/src/main.rs](/desktop/gui/src/main.rs)).

## Arquitectura de la aplicación (`main.rs`)

- **Model-View-Update**: `App::new`, `App::update` (que devuelve `Task<Message>`), `App::view` y `App::subscription` (sondeo de estado en segundo plano, sondeo de `GetHeLive` a ~50 Hz en la página **Keys**, fotogramas de animación y minimizado a la bandeja al cerrar).
- **Opciones CLI**: `--tray` inicia oculta en la bandeja del sistema; `--page <token>` / `-p <token>` abre una página concreta (por ejemplo, `dashboard`, `stats`, `designer`, `settings`, `device`, `logs`, `diagnostics`, `about`, `calibrate`, `keys`).
- **Traspaso de instancia única** ([/desktop/gui/src/single_instance.rs](/desktop/gui/src/single_instance.rs)): Un segundo lanzamiento reenvía su token `--page` por un socket/tubería local privada para enfocar y navegar en la instancia en ejecución, y luego sale.

## Tema y tipografía (`theme.rs`)

[/desktop/gui/src/theme.rs](/desktop/gui/src/theme.rs) replica la paleta de la pantalla del teclado (`firmware/main/ui/core/ui_defaults.c`):
- `BG = #0E0E16`, `SURFACE = #13131E`, `CARD = #1B1B28`, `CARD_HOVER = #24243A`, `BORDER = #2A2A3C`, `PINK = #FF66AA`, `WHITE = #FFFFFF`, `MUTED = #8C8CA6`, `GREEN = #44DD88`, `RED = #FF5566`, `CYAN = #66CCFF`, `YELLOW = #FFCC33`.
- Incrusta `Montserrat-Medium.ttf` y `Montserrat-Bold.ttf` (`SIL OFL 1.1`).

## Diseñador de pantalla (`designer/mod.rs`)

[/desktop/gui/src/designer/mod.rs](/desktop/gui/src/designer/mod.rs) edita los diseños `Idle` y `Playing` (`MAX_WIDGETS = 32`) en un lienzo de vista previa a `2.0x` (`SCALE = 2.0`, `HANDLE = 5` píxeles del teclado) renderizado por [/desktop/crates/opad-ui-preview/src/lib.rs](/desktop/crates/opad-ui-preview/src/lib.rs), que compila el código real de renderizado LVGL en C del firmware para vistas previas exactas al píxel en escritorio.

## Bandeja del sistema (`tray.rs`)

[/desktop/gui/src/tray.rs](/desktop/gui/src/tray.rs) implementa un modelo de bandeja agnóstico al backend (`IconState`: `Warning` / `Connected` / `Disconnected`) respaldado por `ksni` (StatusNotifierItem) en Linux y `tray-icon` en Windows, exponiendo los elementos `TrayAction`: `ShowWindow`, `OpenMonitor`, `SyncNow`, `StartDaemon` y `Quit`.
