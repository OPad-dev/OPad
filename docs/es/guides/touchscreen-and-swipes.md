---
title: Uso de la pantalla táctil y deslizamientos
description: Cómo utilizar el tercer botón táctil para el reintento rápido durante la partida y configurar acciones de deslizamiento direccionales para el control de volumen y pistas.
category: how-to
source_files:
  - /AGENTS.md
  - /firmware/main/input/touch_retry.c
  - /firmware/main/input/gesture.c
  - /desktop/crates/opad-model/src/swipe.rs
  - /desktop/daemon/src/telemetry.rs
---

La pantalla táctil capacitiva de 2,0 pulgadas de OPad funciona como **el tercer botón** (Regla de producto 4). Reconoce el contexto: durante una partida activa de osu!, un dedo apoyado activa el reintento rápido (`` ` ``) y los deslizamientos nunca generan un reintento no deseado; fuera de los beatmaps, los deslizamientos direccionales ejecutan acciones multimedia o de teclado. <!-- src: /AGENTS.md -->

## Comportamiento durante la partida: Reintento rápido

Al jugar un beatmap de osu!: <!-- src: /firmware/main/input/touch_retry.c -->
- **Dedo apoyado**: Coloca y mantén apoyado el dedo en cualquier punto de la superficie de cristal. El firmware observa el movimiento del dedo durante 60 ms (`PLAY_RETRY_STILL_US`). Si el desplazamiento permanece dentro de un radio de inmovilidad de 12 píxeles (`PLAY_RETRY_STILL_PX`), activa el código de tecla de reintento rápido (`` ` `` / `~`). <!-- src: /firmware/main/input/touch_retry.c#PLAY_RETRY_STILL_US -->
- **Mantener para reiniciar**: De forma idéntica a mantener pulsada la tecla `` ` `` en un teclado convencional, mantener el dedo apoyado reinicia el beatmap actual al instante.
- **Protección contra deslizamientos**: El reintento rápido nunca se activa durante un deslizamiento. Un roce accidental o un desplazamiento rápido del dedo no reiniciará tu partida.
- **Pantalla despejada**: Durante la partida activa, no se dibujan insignias ni animaciones de deslizamiento en la pantalla LCD, manteniéndola libre de distracciones visuales.

## Comportamiento fuera de la partida: Deslizamientos direccionales

Al navegar por las pantallas de selección de canciones o utilizar aplicaciones de escritorio, tocar la pantalla no enviará el reintento rápido. En su lugar, los deslizamientos direccionales ejecutan acciones configurables: <!-- src: /firmware/main/input/gesture.c -->

| Dirección del deslizamiento | Acción por defecto | Detalles de comportamiento |
|---|---|---|
| **Hacia arriba** | Subir volumen | Aumenta el volumen interno de osu! si la ventana de osu! tiene el foco; de lo contrario, ajusta el volumen general del sistema. <!-- src: /desktop/crates/opad-model/src/swipe.rs --> |
| **Hacia abajo** | Bajar volumen | Reduce el volumen interno de osu! si la ventana de osu! tiene el foco; de lo contrario, ajusta el volumen general del sistema. <!-- src: /desktop/crates/opad-model/src/swipe.rs --> |
| **Hacia la izquierda** | Pista anterior | Envía el reporte estándar USB HID Consumer para pista anterior. <!-- src: /desktop/crates/opad-model/src/swipe.rs --> |
| **Hacia la derecha** | Siguiente pista | Envía el reporte estándar USB HID Consumer para siguiente pista. <!-- src: /desktop/crates/opad-model/src/swipe.rs --> |

### Mecánica de los deslizamientos
- **Distancia mínima**: Los deslizamientos deben recorrer al menos 24 píxeles (`GESTURE_SWIPE_PX = 24`) desde el punto de contacto. <!-- src: /firmware/main/input/gesture.c -->
- **Bloqueo de eje**: Para evitar activaciones accidentales por movimientos diagonales, el desplazamiento vertical requiere $2 \cdot |dy| \ge 3 \cdot |dx|$, y el horizontal requiere $2 \cdot |dx| \ge 3 \cdot |dy|$. Los movimientos ambiguos esperan un mayor desplazamiento. <!-- src: /firmware/main/input/gesture.c#gesture_feed -->

## Configuración de acciones de deslizamiento personalizadas

Puedes personalizar cada dirección de deslizamiento en `opad-gui`: <!-- src: /desktop/crates/opad-model/src/swipe.rs -->
1. Abre `opad-gui` y dirígete a **Ajustes > Teclado**.
2. Localiza la sección de configuración de **Acciones de deslizamiento**.
3. Selecciona una acción del menú desplegable para cada dirección:
   - **Volumen del sistema / Volumen de osu!**: Control dinámico de volumen predeterminado.
   - **Reproducción multimedia**: Siguiente pista, Pista anterior, Reproducir/Pausar.
   - **Accesos directos de osu!**: Abrir chat (`F9`), Alternar clasificación (`Tab`), Alternar superposición (`Ctrl+O`).
   - **Tecla personalizada**: Cualquier tecla alfanumérica estándar o tecla de función (`F1`–`F12`) con modificadores opcionales (`Ctrl`, `Shift`, `Alt`).
4. Haz clic en **Aplicar cambios**. El demonio transmitirá la configuración actualizada (`SetConfig`) al teclado. <!-- src: /protocol/opad.proto#SetConfig -->

### Configuración mediante opadctl
```bash
# Configurar deslizamiento hacia arriba para subir volumen
opadctl config set swipe-up volume-up

# Configurar deslizamiento hacia la izquierda para pista anterior
opadctl config set swipe-left media-prev
```
