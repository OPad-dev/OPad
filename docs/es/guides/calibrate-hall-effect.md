---
title: Calibrar interruptores Hall Effect
description: Cómo calibrar los interruptores magnéticos de OPad (en el propio teclado o en opad-gui) y ajustar la actuación y el rapid trigger.
sidebar:
  order: 4
---

Un interruptor Hall Effect lleva un imán en el vástago, y un sensor debajo mide cuánto está pulsada la tecla. La calibración registra la lectura de cada tecla en reposo y a fondo, para que el teclado conozca el recorrido de cada una. Hasta que un teclado Hall Effect se calibra, queda bloqueado en su pantalla de calibración: no envía teclas, los deslizamientos están desactivados y un toque solo pulsa el botón de esa pantalla ([decisión D7](../../specs/v2-hall-effect-decisions.md)). <!-- src: /firmware/main/ui/he_cal_ui.c --> <!-- src: /docs/specs/v2-hall-effect-decisions.md -->

## Qué necesitas

- Un OPad con un módulo Hall Effect montado (V1 de 2 teclas o V3 de 3 teclas). <!-- src: /firmware/main/input/he_backend.c -->
- Una mesa plana y estable para que los interruptores no se muevan durante la captura en reposo. <!-- src: /firmware/main/input/he_cal_runner.c -->
- Opcionalmente, la app de escritorio de OPad, para la calibración guiada y para ajustar la actuación y el rapid trigger. La calibración también funciona solo con el teclado, sin la app. <!-- src: /firmware/main/ui/he_cal_ui.c -->

## Calibrar con la app de OPad (recomendado)

Cuando un teclado Hall Effect sin calibrar se conecta a un PC con `opad-daemon` en marcha, la pantalla del teclado muestra **Keys not calibrated** / **Calibrate in the OPad app** / **on your PC**, y el daemon puede abrir `opad-gui` en **Keys**, con la calibración (**Calibrate the keys**) abierta. <!-- src: /firmware/main/ui/he_cal_ui.c#show_lock_screen --> <!-- src: /desktop/gui/src/pages.rs#calibration -->

Para recalibrar un teclado ya calibrado, abre `opad-gui` y haz clic en **Keys** -> **Recalibrate**. La calibración solo empieza si no se está jugando ningún mapa y ningún dedo toca la pantalla del teclado. <!-- src: /desktop/gui/src/keys.rs#key_feel --> <!-- src: /firmware/main/ui/he_cal_ui.c#he_cal_ui_start_calibration -->

1. En **Calibrate the keys** de `opad-gui`, aparta los dedos de todas las teclas y haz clic en **Start**. <!-- src: /desktop/gui/src/pages.rs#calibration -->
2. **Step 1: keys at rest**: mantén los dedos lejos de todas las teclas y no toques el teclado. El teclado deja que las lecturas se estabilicen durante 16 ms y luego promedia 16 ms de lecturas (64 muestras a 4 kHz) para registrar el nivel de reposo de cada tecla y comprobar que es estable y está dentro de rango. <!-- src: /firmware/main/input/he_cal_runner.c#REST_SETTLE_SAMPLES --> <!-- src: /firmware/main/input/he_cal_capture.h#he_cal_capture_cfg_t -->
3. **Step 2: press Key 1 5 times**: pulsa Key 1 con firmeza hasta el fondo y deja que vuelva arriba del todo, 5 veces y despacio. Cada pulsación completada rellena una de las 5 casillas numeradas tanto en `opad-gui` como en la pantalla del teclado. No hay límite de tiempo: tómate tu tiempo. <!-- src: /desktop/gui/src/pages.rs#calibration --> <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen -->
4. **Step 3: press Key 2 5 times**: repite 5 pulsaciones completas y uniformes, soltando del todo, en Key 2. <!-- src: /desktop/gui/src/pages.rs#calibration -->
5. **Step 4: press Key 3 5 times** (solo en el módulo Hall Effect V3 de 3 teclas): repite 5 pulsaciones y liberaciones completas en Key 3. <!-- src: /desktop/gui/src/pages.rs#calibration --> <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen -->
6. Cuando se completa la 5.ª pulsación de la última tecla, el teclado valida el recorrido y la consistencia de los picos (usando la mediana de los 5 picos como lectura a fondo) y escribe el registro de calibración en NVS (`opad_cal` en V1, `opad_cal3` en V3). El teclado muestra **Done!** / **Your keys are ready** durante 1 segundo y las teclas funcionan al instante. <!-- src: /firmware/main/input/he_cal_capture.h --> <!-- src: /firmware/main/input/he_cal.c --> <!-- src: /firmware/main/ui/he_cal_ui.c#he_cal_ui_timer_cb -->

Haz clic en **Cancel** en `opad-gui` en cualquier momento de la captura para abortarla; una recalibración restaura la calibración guardada anterior, mientras que un teclado sin calibrar sigue bloqueado. <!-- src: /desktop/gui/src/pages.rs#calibration --> <!-- src: /firmware/main/ui/he_cal_ui.c#he_cal_ui_cancel_calibration -->

## Calibrar directamente en el teclado (sin app)

Si no hay conectada ninguna app de OPad compatible, el teclado espera 5 segundos tras arrancar (**Looking for the OPad app...**) y luego muestra **For the best setup, get the app:** junto con un botón **Calibrate here** en la pantalla táctil. <!-- src: /firmware/main/ui/he_cal_ui.c#show_lock_screen -->

1. Toca **Calibrate here** en la pantalla táctil y aparta enseguida las manos de las teclas. <!-- src: /firmware/main/ui/he_cal_ui.c#show_lock_screen -->
2. Cuando termine **Calibrating** / **Take your fingers off both keys** (o **Take your fingers off all keys** en V3), sigue las indicaciones de la pantalla: **Press key 1 all the way, 5 times** (**Let it come fully up each time**), luego Key 2 (y Key 3 en V3). Cinco casillas (`1`–`5`) se ponen en verde a medida que se acepta cada pulsación. <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen -->
3. Cuando todas las teclas pasan la validación, el teclado muestra **Done!** / **Your keys are ready** durante 1 segundo y pasa al funcionamiento normal. <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen -->

## Ajustar la actuación y el rapid trigger en opad-gui

Tras la calibración, abre `opad-gui` y haz clic en **Keys** en la barra lateral. Todos los controles deslizantes de profundidad y sensibilidad son un porcentaje del recorrido calibrado de esa tecla, no milímetros, y se mueven en pasos de `0.5 %`. <!-- src: /desktop/gui/src/keys.rs#view --> <!-- src: /desktop/gui/src/keys.rs#setting -->

1. **Vincular o separar las teclas**: de forma predeterminada, los cambios se aplican a todas las teclas a la vez (**Both keys** o **All keys**). Activa **Set keys separately** arriba para configurar Key 1, Key 2 (y Key 3 en V3) por separado. Al desactivarlo, los ajustes de Key 1 se copian a las demás teclas. <!-- src: /desktop/gui/src/keys.rs#view --> <!-- src: /desktop/gui/src/keys.rs#edit_settings -->
2. **Controles básicos**:
   - **ACTUATION POINT**: cuánto tiene que bajar la tecla antes de pulsarse (hasta `95.0 %`; el límite inferior se sube automáticamente para superar el ruido de fondo del sensor salvo que se anule en **Advanced**). <!-- src: /desktop/gui/src/keys.rs#key_controls -->
   - **Rapid trigger**: actívalo para volver a pulsar en cuanto la tecla baja de nuevo y soltar en cuanto sube. <!-- src: /desktop/gui/src/keys.rs#key_controls -->
   - **SENSITIVITY** (se muestra cuando **Rapid trigger** está activado y **Advanced** está oculto): fija a la vez la distancia de movimiento para pulsar y para soltar (`1.0 %`–`50.0 %`). <!-- src: /desktop/gui/src/keys.rs#key_controls -->
   - **Continuous** (se muestra cuando **Rapid trigger** está activado): mantiene el rapid trigger activo en todo el recorrido hasta que la tecla sube por encima del punto de reinicio superior. <!-- src: /desktop/gui/src/keys.rs#key_controls -->
3. **Controles avanzados**: haz clic en **Advanced** (cambia a **Hide advanced**) para mostrar: <!-- src: /desktop/gui/src/keys.rs#view --> <!-- src: /desktop/gui/src/keys.rs#key_controls -->
   - **PRESS SENSITIVITY** (`1.0 %`–`50.0 %`) y **RELEASE SENSITIVITY** (`1.0 %`–`50.0 %`) por separado cuando **Rapid trigger** está activado.
   - **RELEASE POINT** (`1.0 %`–`50.0 %`): histéresis por encima del punto de actuación que se usa cuando **Rapid trigger** está desactivado.
   - **TOP RESET** (`1.0 %`–`50.0 %`, se muestra cuando **Continuous** está activado): profundidad por encima de la cual se reinicia el rapid trigger continuo.
   - **BOTTOM DEAD ZONE** (`0.0 %`–`30.0 %`, `0 = off`): ignora el movimiento extra cerca del fondo del recorrido, para que apretar más contra la carcasa no desplace el pico.
   - **Allow below the noise floor** (si el firmware lo admite): permite ajustes de hasta `1.0 %` sin subirlos al ruido de fondo del sensor; el ruido del sensor puede entonces provocar pulsaciones accidentales.
4. **Vista del interruptor en vivo**: pulsa cada tecla y observa el interruptor en corte y el indicador de recorrido en vivo (`rest`, `press`, `release`/`reset`, `dead zone`, `full`). `opad-gui` consulta `GetHeLive` a ~50 Hz y suaviza el vástago dibujado para que se lea mejor, saltando directamente a la profundidad real al pulsar o soltar; el propio teclado decide las pulsaciones con las lecturas en bruto. La vista en vivo se pausa mientras se juega un mapa. <!-- src: /desktop/gui/src/keys.rs#live_view --> <!-- src: /desktop/gui/src/keys.rs#take_sample -->
5. Haz clic en **Save to pad** para escribir los ajustes en el teclado (o en **Revert** para descartar los cambios sin guardar). <!-- src: /desktop/gui/src/keys.rs#view -->

## Cómo comprobarlo

- En el teclado, la pantalla de bloqueo roja **Keys not calibrated** ha desaparecido y las pulsaciones normales escriben las teclas configuradas (`Z`, `X` y `C` en V3 de 3 teclas). <!-- src: /firmware/main/ui/he_cal_ui.c --> <!-- src: /firmware/main/usb/usb_hid.c -->
- En **Keys** de `opad-gui`, pulsar cada interruptor mueve con suavidad el vástago en corte de `0 %` (`rest`) a `100 %` (`full`) y cambia la etiqueta de estado entre **Released** y **Pressed**. <!-- src: /desktop/gui/src/keys.rs#live_view -->
- Desde un terminal, `opadctl status` muestra `Input Module:     Hall Effect` y `HE Calibrated:    K1: yes, K2: yes` (con `K3: yes` en un módulo V3). <!-- src: /desktop/cli/src/main.rs#run -->

## Qué hacer si no ha funcionado

Si falla una comprobación, el teclado muestra **Calibration failed** y el motivo. Sin la app ofrece un botón **Try again**; con la app dice **Try again in the OPad app**: <!-- src: /firmware/main/ui/he_cal_ui.c#update_calibration_screen --> <!-- src: /firmware/main/input/he_cal_runner.c#he_cal_runner_error_string -->

- **Keep both keys still while resting** (`HE_CAL_ERR_REST_NOT_STILL`): una tecla se movió o vibró durante la captura en reposo. Pon el teclado sobre una superficie estable, aparta los dedos de todas las teclas y vuelve a intentarlo.
- **A key was pressed while resting** (`HE_CAL_ERR_REST_PRESSED`): había una tecla pulsada durante el Step 1. Empieza de nuevo con todas las teclas totalmente sueltas.
- **Sensor reading looks wrong** (`HE_CAL_ERR_REST_OUT_OF_RANGE`) o **The key didn't register** (`HE_CAL_ERR_SIGNAL_NOT_RISING`): comprueba que los interruptores magnéticos (Hall Effect) están bien asentados y, en V3, ejecuta `opadctl pad test --magnet` o **Troubleshooting** -> **Board test** para revisar el cable y los sensores. <!-- src: /desktop/cli/src/main.rs#run -->
- **Press each key all the way down** (`HE_CAL_ERR_SPAN_TOO_SMALL`) o **Press the same way every time** (`HE_CAL_ERR_PEAKS_INCONSISTENT`): lleva cada interruptor hasta el fondo de forma completa y uniforme en las 5 pulsaciones, dejando que el vástago vuelva del todo al reposo entre una y otra.
- **Press exactly 5 times** (`HE_CAL_ERR_TOO_FEW_PEAKS` / `HE_CAL_ERR_TOO_MANY_PEAKS`): haz 5 pulsaciones completas, limpias y separadas, sin rebotes a mitad de pulsación.
- **The OPad app disconnected** o **Could not save calibration**: comprueba la conexión USB y vuelve a intentarlo sin ningún mapa en curso.
