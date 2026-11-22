---
title: Arquitectura del Subsistema GUI
description: Referencia de implementación para opad-gui, arquitectura de estado en iced, bucles de eventos asíncronos por IPC, integración con la bandeja del sistema y previsualización del diseñador de pantallas.
---

La aplicación `opad-gui` está construida con la biblioteca gráfica [iced](https://github.com/iced-rs/iced), siguiendo The Elm Architecture (Model-View-Update). Funciona de forma multiplataforma en Linux y Windows, incorporando un backend de runtime asíncrono Tokio para comunicarse con `opad-daemon` a través de IPC. <!-- src: /desktop/gui/src/main.rs -->

## Application Architecture & Event Loops (main.rs)

El estado de la aplicación está encapsulado en `App`: <!-- src: /desktop/gui/src/main.rs -->

- **`App::update`**: Manejador de eventos puro que responde a interacciones de usuario (`Message`), cambios de navegación, conmutación de ajustes y finalización de tareas asíncronas de IPC (`Task<Message>`).
- **`App::view`**: Generador puro de diseño de interfaz que produce árboles reactivos de elementos para la página activa.
- **`App::subscription`**: Suscripciones a eventos que gestionan pulsos periódicos de sondeo (50 Hz durante la visualización en vivo de Hall Effect, 1 Hz para los latidos en segundo plano del daemon e intercepciones de cierre/minimizado de ventana). <!-- src: /desktop/gui/src/main.rs -->
- **Mutex de Instancia Única (`single_instance.rs`)**:
  Asegura que solo se ejecute una instancia de `opad-gui` por sesión de escritorio con sesión iniciada. Si se vuelve a ejecutar, el segundo proceso señala a la instancia activa a través de una tubería/socket local privado para restaurar y enfocar la ventana existente, y luego finaliza limpiamente.

## Design Language & Theme (theme.rs)

La apariencia visual de la GUI coincide con la pantalla LCD de 2.0 pulgadas del teclado físico (`ui_defaults.c`): <!-- src: /desktop/gui/src/theme.rs -->
- **Paleta**: Fondo vacío casi negro (`BG = #0E0E16`), superficies de elevación oscuras (`SURFACE = #13131E`), bordes sutiles de tarjetas (`CARD = #181826`) y acentos primarios en rosa de osu! (`#FF66AA`). <!-- src: /desktop/gui/src/theme.rs#BG -->
- **Tipografía**: Pesos limpios de la fuente Montserrat en todos los encabezados, paneles de estadísticas y widgets LCD.

## Screen Designer Integration (designer/mod.rs)

El Diseñador de Pantalla permite a los usuarios editar visualmente diseños LCD de 320x240 con precisión de píxel: <!-- src: /desktop/gui/src/designer/mod.rs -->

- **Recompilación del Motor del Firmware (`opad-ui-preview`)**:
  En lugar de escribir un renderizador simulado secundario que diverja del firmware del ESP32, la suite de escritorio compila las rutinas reales de dibujo en C de LVGL del firmware dentro de `opad-ui-preview`. La vista previa del diseñador renderiza una copia exacta a nivel de píxel de lo que mostrará la pantalla ST7789. <!-- src: /desktop/gui/src/designer/mod.rs -->
- **Superposición de Lienzo Interactivo**:
  Renderiza tiradores de arrastre (`HANDLE = 5` píxeles de pad), cuadros delimitadores de selección, líneas guía de alineación y etiquetas de coordenadas escaladas a 2.0x (`SCALE = 2.0`). <!-- src: /desktop/gui/src/designer/mod.rs#SCALE -->

## System Tray Integration (tray.rs)

`opad-gui` se minimiza en la bandeja del sistema en lugar de cerrarse cuando se pulsa el botón de cierre de la ventana: <!-- src: /desktop/gui/src/tray.rs -->
- **Linux**: Utiliza `ksni` (protocolo StatusNotifierItem de KDE), integrándose en entornos de escritorio GNOME (mediante la extensión AppIndicator), KDE Plasma y Wayland. <!-- src: /desktop/gui/src/tray.rs#TrayHandle -->
- **Windows**: Utiliza las APIs Shell NotifyIcon de Windows con bucles de mensajes Win32 personalizados.
- **Acciones del Menú (`TrayAction`)**: Proporciona acciones rápidas: Mostrar Ventana, Abrir Monitor en Vivo, Sincronizar Ahora, Iniciar Daemon y Salir. <!-- src: /desktop/gui/src/tray.rs#TrayAction -->
