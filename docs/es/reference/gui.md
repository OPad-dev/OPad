---
title: Referencia de la Interfaz Gráfica (GUI)
description: Guía de referencia de la aplicación gráfica de escritorio de OPad (opad-gui), sus páginas, diseñador interactivo, diagnósticos e interfaz de calibración de Hall Effect.
category: reference
source_files:
  - /desktop/gui/src/main.rs
  - /desktop/gui/src/pages.rs
  - /desktop/gui/src/keys.rs
  - /desktop/gui/src/diagnostics.rs
  - /desktop/gui/src/designer/mod.rs
  - /desktop/gui/src/stats.rs
---

La interfaz gráfica de usuario de OPad (`opad-gui`) es una aplicación de escritorio basada en iced que proporciona monitorización completa del estado del dispositivo, configuración de ajustes, diseño visual de pantallas LCD con precisión de píxel, diagnósticos interactivos de interruptores y ajuste magnético de actuación para Hall Effect. Se conecta al `opad-daemon` local a través de IPC. <!-- src: /desktop/gui/src/main.rs#parse_page_arg -->

## Navigation Pages

La interfaz gráfica organiza la funcionalidad en páginas principales dedicadas seleccionables desde la barra lateral: <!-- src: /desktop/gui/src/main.rs#Page -->

- **Dashboard**: Vista general del dispositivo en vivo, estado de la conexión, estado de juego activo, contadores de pulsaciones de por vida y detalles del firmware actual. <!-- src: /desktop/gui/src/main.rs#Page::Dashboard -->
- **Keys**: Actuación de Hall Effect, sensibilidad de reinicio, configuración de zona muerta y ajuste de Rapid Trigger con visualización en tiempo real del corte del interruptor. Se muestra dinámicamente cuando se conecta un pad compatible con HE. <!-- src: /desktop/gui/src/keys.rs --> <!-- src: /desktop/gui/src/main.rs#Page::Keys -->
- **Stats**: Analítica de frecuencia de pulsación e histogramas de KPM (pulsaciones por minuto) entre intentos de juego y periodos históricos. <!-- src: /desktop/gui/src/stats.rs --> <!-- src: /desktop/gui/src/main.rs#Page::Stats -->
- **Designer**: Editor visual WYSIWYG de diseño para la pantalla LCD de 320x240 del pad. <!-- src: /desktop/gui/src/designer/mod.rs --> <!-- src: /desktop/gui/src/main.rs#Page::Designer -->
- **Settings**: Paneles de configuración para Teclado, Pantalla, Actualizaciones, Acerca de tosu y parámetros generales del daemon. <!-- src: /desktop/gui/src/pages.rs#SettingsTab --> <!-- src: /desktop/gui/src/main.rs#Page::Settings -->
- **Device**: Ajustes físicos de hardware del pad, asignaciones de pines GPIO, enlaces de gestos táctiles y controles de propiedad. <!-- src: /desktop/gui/src/main.rs#Page::Device -->
- **Diagnostics**: Suite de pruebas de hardware y comunicación (probador de rebote/chatter de interruptores, prueba de latencia del protocolo COM, patrones de prueba de pantalla, autotests de la PCB carrier y exportación de paquete de diagnóstico en un clic). <!-- src: /desktop/gui/src/diagnostics.rs --> <!-- src: /desktop/gui/src/main.rs#Page::Diagnostics -->
- **Logs**: Visor de eventos de registro en tiempo real que transmite eventos de diagnóstico tanto de `opad-daemon` como del búfer circular del pad. <!-- src: /desktop/gui/src/main.rs#Page::Logs -->
- **Calibration**: Asistente interactivo guiado de calibración para los extremos del rango de recorrido magnético de Hall Effect. <!-- src: /desktop/gui/src/main.rs#Page::Calibration -->
- **About**: Versión de la aplicación de escritorio, avisos de licencia y detalles de atribución de terceros. <!-- src: /desktop/gui/src/main.rs#Page::About -->

## Settings Tabs

La página de Ajustes organiza el comportamiento del dispositivo y del escritorio en cinco subpestañas: <!-- src: /desktop/gui/src/pages.rs#SettingsTab -->

### Keypad Tab
- **Algoritmo de Rebote (Debounce)**: Alterna entre rebote anticipado (eager debounce, transmisión de baja latencia con periodo de bloqueo) y rebote diferido (defer debounce, filtrado de contactos ruidosos). <!-- src: /desktop/gui/src/pages.rs#SettingsTab::Keypad -->
- **Ventana de Rebote**: Deslizadores para ajustar la duración del debounce entre 1 ms y 20 ms.
- **Mapeos de Teclas**: Selecciona los códigos de tecla HID enviados al anfitrión para Key 1, Key 2 y las acciones táctiles.

### Display Tab
- **Tiempo de Inactividad de Pantalla**: Configura el intervalo de suspensión de la pantalla (segundos) antes de entrar en reposo por inactividad. <!-- src: /desktop/gui/src/pages.rs#SettingsTab::Display -->
- **Brillo de Retroiluminación**: Nivel de brillo de la pantalla activa (0% a 100%).
- **Animaciones de Resultado**: Menú desplegable de selección de animaciones de celebración (`AnimChoice`) activadas al final de los beatmaps según la calificación. <!-- src: /desktop/gui/src/pages.rs#AnimChoice -->

### Updates Tab
- **Canal de Actualización**: Alterna entre los flujos de publicación `stable` y `beta`. <!-- src: /desktop/gui/src/pages.rs#SettingsTab::Updates -->
- **Comprobación Automática**: Activa o desactiva las comprobaciones periódicas de actualización en segundo plano.
- **Estado de Componentes**: Números de versión en vivo y botones de actualización para la suite de escritorio, tosu y los binarios de firmware.

### About tosu Tab
- **Estado de Integración**: Indicador de conexión con la instancia supervisada de `tosu`. <!-- src: /desktop/gui/src/pages.rs#SettingsTab::AboutTosu -->
- **Estado del Proceso tosu**: Estado del supervisor, salud del escaneo de memoria y telemetría de obtención de metadatos de las canciones.

### Diagnostics Tab
- Acceso directo a las rutinas de autotest de hardware y a la generación del paquete de diagnóstico. <!-- src: /desktop/gui/src/pages.rs#SettingsTab::Diagnostics -->

## Keys Tab (Hall Effect Tuning)

Para pads con módulos de entrada Hall Effect, la pestaña Teclas proporciona personalización profunda del recorrido del interruptor: <!-- src: /desktop/gui/src/keys.rs -->

- **Unidad de Medida**: Conforme al registro de decisiones de hardware D3, todas las distancias de recorrido se expresan como un porcentaje del rango magnético calibrado (en incrementos de 0.1%), nunca en milímetros fijos. <!-- src: /desktop/gui/src/keys.rs -->
- **Punto de Actuación**: El porcentaje de recorrido descendente necesario para registrar una pulsación. <!-- src: /desktop/gui/src/keys.rs -->
- **Rapid Trigger (RT)**:
  - **Sensibilidad de Pulsación**: Distancia que la tecla debe bajar para activar una re-pulsación inmediata mientras ya se encuentra presionada.
  - **Sensibilidad de Liberación**: Distancia que la tecla debe subir para registrar una liberación inmediata.
  - **Zona Muerta**: Banda de filtrado en el punto de reposo y al fondo del recorrido para eliminar ruido y pulsaciones fantasma.
- **Lienzo de Corte del Interruptor en Vivo**: Renderizado gráfico en tiempo real de la posición física del vástago, los umbrales de actuación activos y las lecturas de flujo magnético instantáneas obtenidas de `HeLiveSample`. <!-- src: /desktop/gui/src/keys.rs#HeLiveSample -->

## Screen Designer

El Diseñador de Pantalla (`desktop/gui/src/designer/mod.rs`) permite a los usuarios organizar visualmente widgets en la pantalla LCD de 320x240 del pad: <!-- src: /desktop/gui/src/designer/mod.rs -->

- **Previsualización de Píxeles WYSIWYG**: El lienzo renderiza los widgets utilizando el motor de visualización real del firmware (`opad-ui-preview`), escalado a 2.0x para edición en el escritorio. <!-- src: /desktop/gui/src/designer/mod.rs#SCALE -->
- **Pantallas Soportadas**: Perfiles de diseño independientes para `Idle` (menú/escritorio) y `Playing` (a mitad de un beatmap). <!-- src: /desktop/gui/src/designer/mod.rs -->
- **Colocación de Widgets**: Hasta 32 widgets por pantalla (`MAX_WIDGETS`). Los tiradores permiten arrastrar y redimensionar (`HANDLE = 5` píxeles de pad). <!-- src: /desktop/gui/src/designer/mod.rs#HANDLE -->
- **Propiedades de los Widgets**:
  - **Enlace de Origen**: Fuentes de datos de tosu (UR, combo, precisión, PPM, progreso de la canción, artista/título). <!-- src: /desktop/gui/src/designer/mod.rs -->
  - **Tipografía**: Familia tipográfica, tamaño, precisión decimal, prefijos de etiqueta y unidades.
  - **Estilo**: Alineación (izquierda, centro, derecha), bordes, relleno de fondo y ocultamiento condicional cuando los datos no están disponibles (`FLAG_HIDE_WHEN_EMPTY`). <!-- src: /desktop/gui/src/designer/mod.rs -->

## Diagnostics Suite

La página de Diagnósticos proporciona herramientas de verificación de hardware: <!-- src: /desktop/gui/src/diagnostics.rs -->

1. **Probador de Rebote e Entrada de Interruptores**: Probador de alta resolución de latencia y rebote que detecta cierres de contacto por debajo de 15 ms, avisos sonoros y pruebas de rollover. <!-- src: /desktop/gui/src/diagnostics.rs -->
2. **Prueba de Protocolo y COM de Hardware**: Mide percentiles de latencia de ping de ida y vuelta y valida la resiliencia del analizador de paquetes contra bytes perdidos. <!-- src: /desktop/gui/src/diagnostics.rs -->
3. **Autotest de Pantalla**: Prueba rampas de brillo del LCD, gradientes de color RGB y patrones de prueba de pantalla. <!-- src: /desktop/gui/src/diagnostics.rs -->
4. **Autotest de Placa**: Ejecuta pruebas eléctricas en el dispositivo que verifican buses I2C, comunicación con el controlador táctil CST816S, comunicación con la pantalla ST7789 y muestreo ADC de Hall Effect. <!-- src: /desktop/gui/src/diagnostics.rs -->
5. **Exportación de Paquete de Diagnóstico**: Genera un informe comprimido en zip con detalles de hardware del sistema, registros del daemon, registros del búfer circular del pad, estadísticas USB y estado de calibración para resolución de problemas. <!-- src: /desktop/gui/src/diagnostics.rs -->

## Tap Stats & KPM Analytics

La página de Estadísticas muestra métricas de rendimiento de pulsación del jugador recopiladas durante las partidas: <!-- src: /desktop/gui/src/stats.rs -->

- **Pulsaciones Por Minuto (KPM)**: Mide la frecuencia de pulsación del jugador, distinta del BPM musical del beatmap. <!-- src: /desktop/gui/src/stats.rs -->
- **Estadísticas del Intento**: Gráficos de frecuencia de pulsación en tiempo real, ráfagas máximas y curvas de resistencia para el intento de canción actual o más reciente (`AttemptStats`). <!-- src: /desktop/gui/src/stats.rs#AttemptStats -->
- **Agregación Histórica**: Histogramas históricos de frecuencia de pulsación agregados a lo largo de ventanas de retención configurables (7, 14, 30 o 90 días). <!-- src: /desktop/gui/src/stats.rs#TapHistory -->
