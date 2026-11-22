---
title: Calibración de interruptores Hall Effect
description: Cómo calibrar interruptores magnéticos en OPad (en el teclado o en opad-gui) y ajustar la actuación y Rapid Trigger.
---

Los interruptores magnéticos Hall Effect miden el campo magnético de un imán en el vástago de cada interruptor en lugar de una lámina de contacto metálica. La calibración registra la lectura en posición de reposo y la lectura de fondo de recorrido normal de cada tecla para que el teclado conozca su rango de recorrido magnético. Según el registro de decisiones D7, un teclado Hall Effect sin calibrar queda bloqueado en su pantalla de calibración: no se envían teclas, los deslizamientos táctiles están desactivados y el toque solo opera el botón de calibración hasta completar el proceso. <!-- src: /firmware/main/ui/he_cal_ui.c --> <!-- src: /docs/specs/v2-hall-effect-decisions.md -->

## Qué necesitas

- Un OPad con un módulo Hall Effect instalado (V1 de 2 teclas o V3 de 3 teclas). <!-- src: /firmware/main/input/he_backend.c -->
- Un escritorio plano y estable para que los interruptores no se muevan durante la captura de la posición de reposo. <!-- src: /firmware/main/input/he_cal_runner.c -->
- Opcionalmente, la aplicación de escritorio OPad (`opad-gui` y `opad-daemon`) para la calibración guiada y el ajuste de actuación / Rapid Trigger. La calibración también funciona directamente en el teclado sin la aplicación. <!-- src: /firmware/main/ui/he_cal_ui.c -->

## Calibración con la app de OPad (recomendado)

Cuando un teclado Hall Effect sin calibrar se conecta a un PC con `opad-daemon` en ejecución, la pantalla del teclado muestra **Keys not calibrated** / **Calibrate in the OPad app** / **on your PC**, y el daemon puede abrir `opad-gui` en la página **Hall Effect Calibration**. <!-- src: /firmware/main/ui/he_cal_ui.c#show_lock_screen --> <!-- src: /desktop/gui/src/pages.rs#calibration -->

Para recalibrar un teclado ya calibrado, abre `opad-gui` y haz clic en **Keys** -> **Recalibrate**, o en **Settings** -> **⌨ Keypad** -> **Recalibrate Keys**. La calibración solo comienza mientras no hay ningún mapa en juego (estado `IDLE`) y ningún dedo toca la pantalla del teclado. <!-- src: /desktop/gui/src/keys.rs#view --> <!-- src: /desktop/gui/src/pages.rs --> <!-- src: /firmware/main/ui/he_cal_ui.c#he_cal_ui_start_calibration -->

1. En la página **Hall Effect Calibration** de `opad-gui`, quita los dedos de todas las teclas y haz clic en **Start Calibration**. <!-- src: /desktop/gui/src/pages.rs#calibration -->
2. **Paso 1: Medición de la posición de reposo**: Mantén los dedos fuera de todas las teclas y no toques el teclado. El firmware espera 16 ms a estabilizarse (`REST_SETTLE_SAMPLES = 64` a 4 kHz), luego promedia 64 muestras (otros 16 ms) para registrar el nivel de posición de reposo de cada tecla y verificar que sea estable y esté en rango. <!-- src: /firmware/main/input/he_cal_runner.c#REST_SETTLE_SAMPLES --> <!-- src: /firmware/main/input/he_cal_capture.h#he_cal_capture_cfg_t -->
3. **Paso 2: Pulsa Key 1 (5 veces)**: Pulsa Key 1 firmemente hasta el fondo de recorrido y déjala volver por completo arriba, 5 veces despacio. Cada pulsación completada rellena una de las 5 casillas numeradas tanto en `opad-gui` como en la pantalla del teclado. No hay límite de tiempo — tómate tu tiempo. <!-- src: /desktop/gui/src/pages.rs#calibration --> <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen -->
4. **Paso 3: Pulsa Key 2 (5 veces)**: Repite 5 pulsaciones completas y consistentes con liberación total en Key 2. <!-- src: /desktop/gui/src/pages.rs#calibration -->
5. **Paso 4: Pulsa Key 3 (5 veces)** (solo en módulo Hall Effect V3 de 3 teclas): Repite 5 pulsaciones y liberaciones completas en Key 3. <!-- src: /desktop/gui/src/pages.rs#calibration --> <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen -->
6. Al completarse la 5.ª pulsación de la última tecla, el teclado valida el rango y la consistencia de los picos (usando la mediana de los 5 picos como lectura de fondo de recorrido) y escribe el registro de calibración en NVS (`opad_cal` en V1, `opad_cal3` en V3). El teclado muestra **Done!** / **Your keys are ready** durante 1 segundo y desbloquea las teclas inmediatamente. <!-- src: /firmware/main/input/he_cal_capture.h --> <!-- src: /firmware/main/input/he_cal.c --> <!-- src: /firmware/main/ui/he_cal_ui.c#he_cal_ui_timer_cb -->

Haz clic en **Cancel** en `opad-gui` en cualquier momento durante la captura para abortar; una recalibración restaura la calibración previa guardada, mientras que un teclado sin calibrar permanece bloqueado. <!-- src: /desktop/gui/src/pages.rs#calibration --> <!-- src: /firmware/main/ui/he_cal_ui.c#he_cal_ui_cancel_calibration -->

## Calibración directa en el teclado (sin app)

Si no hay ninguna app de OPad compatible conectada, el teclado espera 5 segundos tras arrancar (**Looking for the OPad app...**), y después muestra **For the best setup, get the app:** junto con un botón **Calibrate here** en la pantalla táctil. <!-- src: /firmware/main/ui/he_cal_ui.c#show_lock_screen -->

1. Toca **Calibrate here** en la pantalla táctil y retira de inmediato las manos de las teclas. <!-- src: /firmware/main/ui/he_cal_ui.c#show_lock_screen -->
2. Cuando termine **Calibrating** / **Take your fingers off both keys** (o **Take your fingers off all keys** en V3), sigue las indicaciones en pantalla: **Press key 1 all the way, 5 times** (**Let it come fully up each time**), luego Key 2 (y Key 3 en V3). Cinco casillas (`1`–`5`) se ponen en verde a medida que se acepta cada pulsación. <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen -->
3. Cuando todas las teclas superan la validación, el teclado muestra **Done!** / **Your keys are ready** durante 1 segundo y pasa a funcionamiento normal. <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen -->

## Ajuste de actuación y Rapid Trigger en opad-gui

Tras calibrar, abre `opad-gui` y haz clic en **Keys** en la barra lateral. Todos los deslizadores de profundidad y sensibilidad utilizan un porcentaje del rango magnético calibrado de cada tecla (nunca milímetros fijos, según el registro de decisiones D3), ajustado en pasos de `0.5 %` (`step(5)` en unidades de `0.1 %`). <!-- src: /desktop/gui/src/keys.rs#view --> <!-- src: /desktop/gui/src/keys.rs#setting -->

1. **Vincular o separar teclas**: Por defecto, las ediciones se aplican a todas las teclas juntas (**Both keys** o **All keys**). Activa **Set keys separately** arriba para configurar Key 1, Key 2 (y Key 3 en V3) de forma independiente. Desactivarlo copia los ajustes de Key 1 a las demás teclas. <!-- src: /desktop/gui/src/keys.rs#view --> <!-- src: /desktop/gui/src/keys.rs#edit_settings -->
2. **Controles básicos**:
   - **ACTUATION POINT**: Profundidad que debe bajar la tecla antes de pulsarse (hasta `95.0 %`; el límite inferior sube automáticamente para superar el piso de ruido del sensor salvo que se anule en **Advanced**). <!-- src: /desktop/gui/src/keys.rs#key_controls -->
   - **Rapid trigger**: Actívalo para volver a pulsar en cuanto la tecla baje de nuevo y soltar en cuanto suba. <!-- src: /desktop/gui/src/keys.rs#key_controls -->
   - **SENSITIVITY** (visible con **Rapid trigger** activado y **Advanced** oculto): Fija juntas la distancia de movimiento para pulsar y soltar (`1.0 %`–`50.0 %`). <!-- src: /desktop/gui/src/keys.rs#key_controls -->
   - **Continuous** (visible con **Rapid trigger** activado): Mantiene Rapid Trigger activo durante todo el ascenso hasta que la tecla supera el punto de reinicio superior. <!-- src: /desktop/gui/src/keys.rs#key_controls -->
3. **Controles avanzados**: Haz clic en **Advanced** (cambia a **Hide advanced**) para mostrar: <!-- src: /desktop/gui/src/keys.rs#view --> <!-- src: /desktop/gui/src/keys.rs#key_controls -->
   - **PRESS SENSITIVITY** (`1.0 %`–`50.0 %`) y **RELEASE SENSITIVITY** (`1.0 %`–`50.0 %`) por separado cuando **Rapid trigger** está activado.
   - **RELEASE POINT** (`1.0 %`–`50.0 %`): Histéresis por encima del punto de actuación usada cuando **Rapid trigger** está desactivado.
   - **TOP RESET** (`1.0 %`–`50.0 %`, visible con **Continuous** activado): Profundidad por encima de la cual se reinicia Rapid Trigger continuo.
   - **BOTTOM DEAD ZONE** (`0.0 %`–`30.0 %`, `0 = off`): Zona muerta inferior que ignora el movimiento adicional cerca del fondo de recorrido para que presionar con más fuerza contra la carcasa no desplace el pico.
   - **Allow below the noise floor** (cuando lo admite el firmware): Permite ajustes hasta `1.0 %` sin elevarlos al piso de ruido del sensor; el ruido del sensor puede provocar pulsaciones accidentales.
4. **Vista en vivo del interruptor**: Pulsa cada tecla y observa el corte transversal del interruptor y el medidor de recorrido en vivo (`rest`, `press`, `release`/`reset`, `dead zone`, `full`). `opad-gui` consulta `GetHeLive` a ~50 Hz y suaviza el vástago dibujado para mayor legibilidad, saltando directamente a la profundidad real al pulsar/soltar; el teclado decide las pulsaciones con las lecturas crudas. La vista en vivo se pausa mientras juegas un mapa. <!-- src: /desktop/gui/src/keys.rs#live_view --> <!-- src: /desktop/gui/src/keys.rs#take_sample -->
5. Haz clic en **Save to pad** para escribir los ajustes en el teclado (o en **Revert** para descartar cambios sin guardar). <!-- src: /desktop/gui/src/keys.rs#view -->

## Cómo comprobarlo

- En el teclado, desaparece la pantalla roja de bloqueo **Keys not calibrated** y las pulsaciones normales escriben tus teclas configuradas (`Z`, `X`, y `C` en V3 de 3 teclas). <!-- src: /firmware/main/ui/he_cal_ui.c --> <!-- src: /firmware/main/usb/usb_hid.c -->
- En **Keys** de `opad-gui`, pulsar cada interruptor mueve suavemente el vástago de `0 %` (`rest`) a `100 %` (`full`) y alterna la etiqueta de estado entre **Released** y **Pressed**. <!-- src: /desktop/gui/src/keys.rs#live_view -->
- Desde un terminal, `opadctl status` muestra cada tecla Hall Effect activa como calibrada con su rango ADC crudo `rest..full`. <!-- src: /desktop/cli/src/main.rs#Commands::Status -->

## Qué hacer si no funcionó

Si falla la validación, el teclado muestra **Calibration failed** con el motivo concreto durante 2 segundos (y un botón **Try again** en modo autónomo, o **Try again in the OPad app** cuando está conectado a la app): <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen --> <!-- src: /firmware/main/input/he_cal_runner.c#he_cal_runner_error_string -->

- **Keep both keys still while resting** (`HE_CAL_ERR_REST_NOT_STILL`): Alguna tecla se movió o vibró durante la captura de posición de reposo. Coloca el teclado sobre una superficie firme, quita los dedos de todas las teclas y reintenta.
- **A key was pressed while resting** (`HE_CAL_ERR_REST_PRESSED`): Alguna tecla estaba parcialmente pulsada en el Paso 1 y subió más en el Paso 2. Empieza de nuevo con todas las teclas completamente sueltas.
- **Sensor reading looks wrong** (`HE_CAL_ERR_REST_OUT_OF_RANGE`) o **The key didn't register** (`HE_CAL_ERR_SIGNAL_NOT_RISING`): Comprueba que los interruptores magnéticos (Hall Effect) estén bien asentados y, en V3, ejecuta `opadctl board-test --magnet` o **Diagnostics** -> **🔧 PCB Test** para comprobar el cable y los sensores. <!-- src: /desktop/cli/src/main.rs#Commands::BoardTest -->
- **Press each key all the way down** (`HE_CAL_ERR_SPAN_TOO_SMALL`) o **Press the same way every time** (`HE_CAL_ERR_PEAKS_INCONSISTENT`): Lleva cada interruptor al fondo de recorrido de forma completa y consistente en las 5 pulsaciones, dejando que el vástago vuelva del todo a la posición de reposo entre pulsaciones.
- **Press exactly 5 times** (`HE_CAL_ERR_TOO_FEW_PEAKS` / `HE_CAL_ERR_TOO_MANY_PEAKS`): Realiza 5 pulsaciones limpias y diferenciadas sin rebotar a mitad de recorrido.
- **The OPad app disconnected** o **Could not save calibration**: Vuelve a conectar el cable USB y reintenta sin ningún mapa en curso (`IDLE`).
