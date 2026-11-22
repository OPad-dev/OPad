---
title: Calibración de interruptores Hall Effect
description: Guía paso a paso de calibración para interruptores magnéticos en OPad, registro de niveles de reposo y fondo, y ajuste de rapid trigger en opad-gui.
category: how-to
source_files:
  - /firmware/main/input/he_cal.c
  - /firmware/main/ui/he_cal_ui.c
  - /firmware/main/input/he_cal_runner.c
  - /desktop/gui/src/keys.rs
  - /desktop/gui/src/he_monitor.rs
  - /protocol/opad.proto
---

Los interruptores magnéticos Hall Effect requieren calibración para establecer los límites del flujo magnético entre la posición de reposo del vástago (arriba) y el estado de pulsación a fondo (abajo). De acuerdo con el registro normativo de decisiones D7, un teclado Hall Effect sin calibrar queda bloqueado en su asistente de calibración en pantalla hasta que se complete el proceso. <!-- src: /firmware/main/ui/he_cal_ui.c -->

## Guía paso a paso de calibración

La calibración puede realizarse directamente en la pantalla táctil del teclado o guiarse mediante `opad-gui`. <!-- src: /firmware/main/input/he_cal_runner.c -->

### Paso 1: Captura de la posición de reposo
1. Asegúrate de que ningún dedo u objeto esté tocando los vástagos de los interruptores.
2. Toca el botón **Start Calibration** en la pantalla táctil (o haz clic en **Start Calibration** en la página Calibración de `opad-gui`). <!-- src: /firmware/main/ui/he_cal_ui.c -->
3. El teclado recoge 64 muestras por tecla durante 16 ms (`REST_SETTLE_SAMPLES = 64`) para calcular el nivel magnético de referencia en reposo. <!-- src: /firmware/main/input/he_cal_runner.c#REST_SETTLE_SAMPLES -->
4. Si el vástago de un interruptor se mueve o presenta ruido durante este paso, el ejecutor de calibración informará de `HE_CAL_ERR_REST_NOT_STILL`. Asegúrate de que el teclado repose sobre una mesa estable. <!-- src: /firmware/main/ui/he_cal_ui.c -->

### Paso 2: Captura del fondo de la Tecla 1
1. La pantalla indicará: `Press Key 1 firmly to the bottom`.
2. Presiona la Tecla 1 hasta el fondo hasta que haga contacto físico con la carcasa del interruptor y mantenla firme durante 1 segundo.
3. El teclado registra la lectura magnética máxima y emite un pitido de confirmación o hace avanzar la barra de progreso visual.

### Paso 3: Captura del fondo de la Tecla 2
1. La pantalla indicará: `Press Key 2 firmly to the bottom`.
2. Presiona la Tecla 2 completamente hacia abajo y mantenla pulsada durante 1 segundo.
3. Si utilizas un módulo OPad V3 de 3 teclas, repite el proceso para la Tecla 3.

### Paso 4: Verificación y guardado
1. El teclado valida que la diferencia del recorrido magnético entre el reposo y el fondo supere el umbral mínimo aceptable de recorrido.
2. Si la validación es correcta, el ejecutor de calibración escribe el nuevo registro de calibración A/B en NVS (`s_store12` en `he_cal.c`). <!-- src: /firmware/main/input/he_cal.c -->
3. La pantalla de bloqueo de calibración se cierra, devolviendo el teclado a su modo operativo habitual.

## Ajuste de actuación y Rapid Trigger en opad-gui

Una vez calibrado, puedes ajustar la respuesta del interruptor en `opad-gui`: <!-- src: /desktop/gui/src/keys.rs -->

1. Abre `opad-gui` y haz clic en **Keys** en la barra lateral.
2. **Comprender las unidades**: De acuerdo con el registro de decisiones D3, todos los controles deslizantes de recorrido representan un porcentaje del recorrido magnético calibrado (en incrementos del 0,1%), nunca milímetros fijos. <!-- src: /desktop/gui/src/keys.rs -->
3. **Punto de actuación**:
   - Establece el porcentaje de recorrido descendente requerido para activar el interruptor (p. ej., `20.0%`).
4. **Ajustes de Rapid Trigger**:
   - Activa **Rapid Trigger**.
   - **Sensibilidad de pulsación**: Distancia que el vástago debe descender para volver a registrar una activación mientras se mantiene pulsado (p. ej., el equivalente a `0.2 mm`, representado como `5.0%`).
   - **Sensibilidad de liberación**: Distancia que el vástago debe ascender para activar una liberación inmediata (p. ej., `5.0%`).
   - **Zona muerta**: Márgenes de zona muerta superior e inferior (por defecto `5.0%`) para eliminar fluctuaciones magnéticas cuando la tecla está totalmente liberada o tocando fondo.
5. **Lienzo de visualización del corte del interruptor en vivo**:
   - Al pulsar físicamente el interruptor, observa el lienzo visualizador en tiempo real en `opad-gui`. El visualizador sigue el recorrido del vástago, los umbrales de actuación y los puntos de liberación sondeados desde `HeLiveSample` a 50 Hz. <!-- src: /desktop/gui/src/keys.rs -->
6. Haz clic en **Save Settings** para guardar la configuración de rapid trigger en la memoria del teclado.
