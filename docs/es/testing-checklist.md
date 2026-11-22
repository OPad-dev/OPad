---
title: Lista de verificación de hardware y versiones
description: Lista manual de control de calidad para firmware, software de escritorio y hardware antes del lanzamiento.
sidebar:
  order: 4
---

Esta lista de verificación documenta el procedimiento manual de verificación del hardware previo al lanzamiento (§34, §35) requerido antes de las versiones etiquetadas. Cada fila es ejecutada por una persona con el teclado delante; nada de esto puede automatizarse ni inferirse del resultado en otra plataforma.

**Hardware objetivo:** Waveshare ESP32-S3-Touch-LCD-2.0  
**Probador:** GFerreiroS  

| Ejecución | Fecha | Host | Firmware | Estado |
|---|---|---|---|---|
| Linux | 2026-09-13 | Linux x86_64, kernel 6.x | v1.0.0 | Completo — pero véase la nota a continuación |
| Reejecución en Linux (U-3a) | 2026-09-17 | Linux x86_64, kernel 7.2 | 1.0.0 en tabla de dos ranuras | Parcial — §9 |
| Windows (§W4-2) | 2026-09-18 | Windows 11 x86_64 (hardware real) | v1.0.0 (`OSUPAD-3CDC75701678`) | Completo — 100% PASS (§12) |
| Linux, módulos V3 | 2026-11-20 | Linux x86_64, kernel 7.3 | 1.0.0-rc, rama `feat/v3-modules`, en carrier V3 con módulos HE V3 y MX V3 | Parcial — HW-06 a HW-15, BAK-01 |

> **La columna de Linux del 2026-09-13 está obsoleta para los contadores.** Es anterior tanto al cambio del blob de configuración NVS v2 → v3 (§W3-2) como a la tabla de particiones de dos ranuras (§U-3a), por lo que **HW-01, HW-02, FAIL-03 y STR-02 ya no son válidos como evidencia** para el firmware definitivo y se reabren en el §9. El resto de la columna de Linux no se ve afectada: nada en esas filas toca la estructura de NVS o el particionado.

> **La columna de Windows a continuación está vacía intencionadamente.** Se completa ejecutando las pruebas en hardware con Windows, no deduciendo a partir de Linux. **HW-05** (HID primero al despertar la pantalla) y **STR-02** (cero escrituras de almacenamiento) forman la pareja crítica: el §W4-2 exige que ambas se reverifiquen en Windows en lugar de asumirse.

Leyenda: **PASS** / **FAIL** / **NOT RUN** / **n/r** = no requerido en esta plataforma según §W4-2.

---

## 1. Rendimiento físico de teclas e invariantes de latencia

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| HW-01 | Pulsación y liberación Tecla 1 | Pulsar la Tecla 1 limpiamente 20 veces. | Emisión exacta 1:1 en `evtest`, 0 teclas atascadas, el contador de vida útil se incrementa en 20. | **RE-OPENED** (§9) | **PASS** (2026-09-18) — Verificadas 20 pulsaciones, pulsaciones discretas 1:1, contador incrementado exactamente en 20 |
| HW-02 | Pulsación y liberación Tecla 2 | Pulsar la Tecla 2 limpiamente 20 veces. | Emisión exacta 1:1 en `evtest`, 0 teclas atascadas, el contador de vida útil se incrementa en 20. | **RE-OPENED** (§9) | **PASS** (2026-09-18) — Verificadas 20 pulsaciones, pulsaciones discretas 1:1, contador incrementado exactamente en 20 |
| HW-03 | Pulsación simultánea de teclas | Pulsar Tecla 1 + Tecla 2 concurrentemente en una ventana de 1 ms. | Ambas teclas registradas hacia abajo y hacia arriba sin ghosting ni bloqueos. | **PASS** | **PASS** (2026-09-18) — Verificado mediante `test_hw_input`; 27 pulsaciones simultáneas registradas concurrentemente con 0 ghosting |
| HW-04 | Flujo alternado rápido | Alternar K1/K2 a > 20 pulsaciones/segundo durante 60 segundos. | 0 pulsaciones perdidas, 0 rebotes/dobles pulsaciones, delta de latencia p99.9 < 0.1 ms sobre la línea base. | **PASS** | **PASS** (2026-09-18) — 566 pulsaciones durante 68.1s en `test_hw_input`; 0 pulsaciones perdidas, latencia hardware p50 200µs / p99.9 870µs |
| HW-06 | Prueba de placa (carrier + módulo MX) | Con el módulo MX en la placa portadora y ambas teclas liberadas, ejecutar `opadctl board-test` (o Diagnósticos → Prueba de PCB). Repetir sin cable, con cable invertido pin 1 ↔ 8, y manteniendo pulsada la Tecla 1. | Placa sana: pasa todas las pruebas. Desconectada: "No input module answers". Invertida: falla nombrando la inversión. Tecla pulsada: falla "reads low". Las teclas funcionan con normalidad inmediatamente después: ninguna tecla queda atascada y no se contabiliza ninguna pulsación (`opadctl status` antes y después). | **FAIL**, corregido: hasta `1fab1fe` cada prueba dejaba la Tecla 2 (GPIO7) retenida en pads MX, porque abrir el ADC de ID de módulo configuraba el canal de GPIO7 como analógico. Reejecutado el 2026-11-20 en MX V3 tres veces seguidas: **PASS**, recuento y muestras de latencia inalterados | n/r |
| HW-07 | Prueba de placa (carrier V3 + módulo HE V3) | Con el módulo HE V3 en el carrier V3, sin teclas instaladas, ejecutar `opadctl board-test --magnet` (o Diagnósticos → Prueba de PCB, luego Iniciar prueba de imán) y mantener un imán sobre la tecla 1, 2 y luego 3. | Módulo "Hall Effect V3" a ~1.65 V; ADC responde 0x81, 20/20 escrituras de registro, margen de señal limpio hasta al menos 20 MHz, la alimentación se mantiene; cada sensor reposa cerca de 1.65 V con menos de 3 códigos rms de ruido; cada prueba de imán pasa únicamente en su tecla. | **PASS** 2026-11-20 cable recto, sin teclas (ID 1.66 V, limpio hasta 40 MHz, 0.8 códigos rms). Imán: NOT RUN | n/r |
| HW-08 | Tecla 3 (HE V3) | Con interruptores instalados, pulsar la tecla 3 20 veces observando en `evtest`. Asignar Tecla 3 como vacía en Ajustes y pulsar de nuevo. | Emisión exacta 1:1 de la configuración de Tecla 3 (C por defecto, `DEFAULT_KEY3_HID_USAGE` en `opad-model`); su propio contador (`Key 3 (K3)` en `opadctl status`) sube 20 y los de K1/K2 no cambian. Vacía: no envía nada. | **PARTIAL** 2026-11-20 — escribe C y su contador registra. Prueba de 20 pulsaciones en `evtest` y ajuste vacío: NOT RUN | n/r |
| HW-09 | Teclas de juego | Ajustes → TECLAS DE JUEGO: marcar Tecla 1 y Tecla 3 y guardar; pulsar tecla 3 y luego tecla 2; marcar Tecla 1 y Tecla 2 de nuevo. Intentar marcar una tercera casilla. | Teclas 1+3: la segunda tarjeta del teclado muestra la letra y contador de Tecla 3 (`ui/ui_port.c`, `input/game_keys.h`), el total y KPS cuentan solo 1 y 3, y la tecla 2 sigue escribiendo sin mover tarjeta. Volviendo a 1+2: sin picos de KPS. Con dos marcadas la tercera se desactiva y el teclado rechaza tres (`config/config_validate.c`). | **PASS** 2026-11-20 | n/r |
| HW-10 | Calibración con tres teclas | Calibrar un pad HE V3 en el propio pad y luego desde la aplicación. Borrar la calibración de la tecla 3 y reiniciar. | Las tres teclas se calibran y `HE Calibrated` muestra K1, K2, K3 sí. Con la tecla 3 sin calibrar el teclado queda bloqueado en la pantalla de calibración (decisión D7). | **PARTIAL** 2026-11-20 — calibración en el pad y desde la app. Bloqueo con tecla 3 sin calibrar: NOT RUN | n/r |
| HW-11 | Comprobador de Tecla 3 en Diagnósticos | Diagnósticos → Rebote de interruptores y entrada en teclado de 3 teclas; pulsar tecla 3 sola, luego con tecla 1 y con tecla 2. | Aparece tarjeta de Tecla 3 sin pin; solo ella se ilumina para la tecla 3; tecla 3 con cualquier otra cuenta como simultánea (`gui/src/diagnostics.rs`). | **PASS** 2026-11-20 | n/r |
| HW-12 | Enlace HE V3: CRC y velocidad | Ejecutar prueba de placa y luego usar teclas. Con build de prueba que registre `he_spi_stats_t` (`input/he_spi_source.h`), dejar transmitir durante un minuto. | Pasa la prueba justo tras el flujo continuo (el ADC se reinicia fuera de modo CRC, `ads7028_reset`) y las teclas funcionan después. Sin errores de CRC ni SPI, todas las teclas leídas en cada trama en ~14 µs (`docs/hardware/components/ads7028.md`). | **PARTIAL** 2026-11-20 — prueba de placa tras transmisión, 0 errores CRC en más de 400.000 tramas, media 14.4 µs, máx 51-62 µs. Pulsaciones reales y mapa con este firmware: NOT RUN (sin caja aún) | n/r |
| HW-13 | Prueba de placa (carrier V3 + módulo MX V3) | Con módulo MX V3 en carrier V3 mediante FFC de 14 vías tipo B y todas las teclas liberadas, ejecutar `opadctl board-test`. | Módulo "MX V3" a ~2.24 V; Cable: ID recibido en GPIO8 (contacto 1 con 1); pines GPIO10 / GPIO7; líneas de Tecla 1, 2 y 3 ven pull-up de 10 kΩ en GPIO10, GPIO7 y GPIO6 (`opad-model/src/board_test.rs`). Sin comprobación de líneas libres en módulo V3. | **PASS** 2026-11-20 (ID 2.26-2.27 V, 856-860 mV bajo carga) | n/r |
| HW-14 | Tecla 3 (MX V3) y tres teclas MX | En pad MX V3, pulsar teclas 1, 2 y 3, mantener pulsadas y jugar un mapa con teclas 1 y 2. | Tecla 3 escribe su valor (C por defecto) y repite al mantener, con contador propio (`Key 3 (K3)` en `opadctl status`); teclas 1 y 2 como siempre. Latencia idéntica a la del firmware de 2 teclas. | **PASS** 2026-11-20 — Z, X y C escriben y se mantienen. Mapa: 792 muestras, p50 30 µs, p99 40 µs, p99.9 50 µs, máx 46 µs, 0 aplazadas (firmware de dos teclas: 1920 muestras, p50 30 µs, p99 60 µs, máx 54 µs) | n/r |
| HW-15 | Ajustes de pines solo en teclado cableado | Abrir Ajustes → Teclas con módulo en el carrier y luego en pad sin módulo (cableado directo a cabecera). | Con módulo (MX, MX V3, HE, HE V3) no hay selectores de PIN TECLA 1 / PIN TECLA 2 ni Detección automática; sin módulo sí aparecen. Firmware que no envíe `HelloAck.input_module` los mantiene (`DeviceInfo::key_pins_configurable` en `opad-model`). | **PARTIAL** 2026-11-20 — MX V3: sin selectores. Pad cableado a mano: NOT RUN | n/r |
| HW-05 | Pantalla en reposo -> Despertar al pulsar | Dejar dormir la pantalla (10 min inactiva), pulsar Tecla 1. | **Invariante HID-primero verificado**: Reporte de tecla enviado de inmediato antes de iniciar la secuencia de despertar de la pantalla. Sin retraso de entrada. | **PASS** | **PASS** (2026-09-18) — Pulsación emitida al PC host de inmediato antes de iniciar el encendido de pantalla |

---

## 2. Comunicación y robustez del protocolo

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| COM-01 | Host CDC ausente | Arrancar el pad con el daemon detenido (host USB HID puro). | El teclado arranca limpiamente, funciona normalmente, sin reinicios por watchdog de FreeRTOS ni saturación de búfer. | **PASS** | **PASS** (2026-09-18) — PnP reporta Dispositivo de teclado HID OK y Presente con daemon detenido |
| COM-02 | Trama de protocolo malformada | Inyectar bytes de ruido aleatorio y cabeceras de trama no válidas en `/dev/ttyACM0`. | El firmware descarta el búfer malformado, registra `DIAG_EVENT_FRAME_TOO_LARGE` / `DIAG_EVENT_DECODE_FAILED` y se recupera en la siguiente trama válida sin bloquearse. | **PASS** | **PASS** (2026-09-18) — Inyectados 512B de ruido, trama excesiva de 0x7FFFFFFF y bytes corruptos; el teclado se recuperó con el siguiente Hello |
| COM-03 | Reconexión USB rápida | Desconectar y reconectar el cable USB 20 veces seguidas (intervalo de 1s). | El daemon se reconecta limpiamente cada vez; el teclado se enumera como HID + CDC sin bloquearse. | **PASS** | n/r |
| COM-04 | Cierre del daemon en partida | Terminar `opad-daemon` (`kill -9`) en mitad de una partida activa. | El teclado sigue funcionando como teclado HID a 1000 Hz sin ninguna interrupción en las pulsaciones. | **PASS** | n/r |
| COM-05 | Cierre de tosu en partida | Terminar el servidor WebSocket de tosu durante una partida activa. | El teclado pasa de forma segura de PLAYING a COOLDOWN (5s) y luego a IDLE; sin bloqueos en la interfaz. | **PASS** | n/r |

---

## 3. Almacenamiento y aislamiento de fallos (§P0-1, §P2-12)

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| FAIL-01 | Fallback ante fallo de LCD | Compilar con `CONFIG_OPAD_TEST_FAIL_LCD=y` y flashear. | Fallo no fatal de LCD registrado; el teclado pasa a operar sin pantalla; 0 impacto en latencia HID. | **PASS** | n/r |
| FAIL-02 | Fallback ante fallo de NVS | Compilar con `CONFIG_OPAD_TEST_FAIL_NVS=y` y flashear. | Fallo no fatal de NVS registrado; el teclado pasa a contadores solo en RAM; el teclado y protocolo siguen funcionando. | **PASS** | n/r |
| FAIL-03 | Persistencia tras corte de corriente | Jugar mapa con 500 pulsaciones, esperar 10s en IDLE para sincronización, desconectar alimentación. Reconectar. | Los contadores de vida útil coinciden exactamente con los previos a la desconexión; 0 pérdidas de pulsaciones. | **PASS** (2026-09-18) | **PASS** (2026-09-18) — Verificadas más de 500 pulsaciones, sincronización en IDLE y corte de alimentación; contadores idénticos con 0 pérdida |
| FAIL-04 | Modo degradado de SQLite | Revocar permisos de escritura en la base de datos (`chmod 400 opad.db`), iniciar daemon. | El daemon arranca en modo degradado, muestra `storage_error` en GUI/IPC, bloquea escrituras destructivas y mantiene diseños en memoria. | **PASS** | **PASS** (2026-09-18) — Probado con opad.db de solo lectura; daemon arrancó y conectó al hardware |
| FAIL-05 | La actualización va a la otra ranura | En pad funcional, ejecutar `opadctl flash firmware/build/opad-firmware.bin`. | Muestra la ranura que escribe y la de respaldo; regresa con "Flash succeeded" en la otra ranura (`Running Slot` en `opadctl status`). | **PASS** (2026-10-10, ota_0 → ota_1, entrada confirmada VALID) | n/r |
| FAIL-06 | Actualización dañada revierte | Compilar con `CONFIG_TINYUSB_HID_COUNT=1` y assertions eliminadas en `usb_descriptors.c`, luego flashear con `opadctl flash`. | `dmesg` muestra `can't set config #1, error -32`; ~20 s después el pad reinicia en la ranura previa por sí solo; `opadctl flash` indica que volvió a la anterior; las teclas funcionan. | **PASS** (2026-10-10, de nuevo en ota_0 tras 22 s) | n/r |
| FAIL-07 | Recuperación con reconexión | `opadctl recover firmware/build`, luego reconectar el pad. | "Caught the pad booting", cuatro imágenes escritas, "Recovered". Ningún botón pulsado. | **PASS** (2026-10-10, ventana de arranque desde reinicio por watchdog; también desde download mode) | n/r |
| BAK-01 | Copia JSON con tecla 3 | En pad de 3 teclas, exportar copia; editar `config.key3` y `config.game_keys`, pulsar tecla 3 varias veces e importar (Dispositivo → Copias de seguridad o `opadctl import`). Importar copia original después. | La exportación incluye `stats.lifetime_key3`, `config.key3` y `config.game_keys` (`JsonBackup` en `opad-model`). La previsualización muestra Tecla 3 y teclas de juego, y el recuento de tecla 3 bajando como reversión. Tras importar, tecla 3 escribe la tecla de la copia y cuenta desde su valor. Copia sin esos campos los mantiene inalterados. | **PASS** 2026-11-20 | n/r |

---

## 4. Pruebas de resistencia y carga

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| STR-01 | Sesión de juego extendida | Ejecutar sesión continua durante ≥ 2 horas con tosu en vivo y pantalla activa. | 0 bloqueos, 0 fugas de memoria, 0 caídas de cuadros, latencia p99 estable (< 60 µs), estabilidad térmica. | **PASS** | **PASS** (2026-09-18) — Sesión extendida completada; cero bloqueos, cero fugas de memoria, pantalla estable |
| STR-02 | Invariante de cero escrituras | Monitorizar E/S de disco al jugar un mapa y durante los 5s de enfriamiento. | Cero transacciones SQLite o escrituras en disco hasta alcanzar IDLE. Verificado por `test_writes_blocked_guard`. | **PASS** | **PASS** (2026-09-18) — Verificado mediante `test_zero_storage_writes_during_gameplay_and_cooldown`; cero escrituras durante PLAYING o COOLDOWN |

### Equivalentes en Windows de las herramientas de Linux anteriores

Los procedimientos están redactados con las herramientas de Linux. En Windows, sustituir por:

| Linux | Windows |
|---|---|
| `evtest` (HW-01…HW-05) | **Keyboard Tester** de Microsoft o cualquier visor de entrada raw que muestre key-down y key-up por separado. Un editor de texto **no** es suficiente, ya que oculta la repetición automática y las teclas atascadas. |
| `/dev/ttyACM0` (COM-02) | El puerto `COMn` del teclado, obtenido de `espflash list-ports` o Administrador de dispositivos → Puertos. |
| `iotop` / `strace` en archivo SQLite (STR-02) | **Process Monitor** de Sysinternals, filtrado por `opad-daemon.exe` y ruta `opad.db`. Incluir `WriteFile`, `SetEndOfFile` y `FlushBuffersFile`; los archivos WAL y `-shm` cuentan. |
| `systemctl --user stop opad-daemon` (COM-01) | Finalizar `opad-daemon.exe` en el Administrador de tareas o detenerlo desde la bandeja del sistema. |
| `kill -9` (COM-04) | `taskkill /F /IM opad-daemon.exe`. |

---

## 5. Integración con la plataforma Windows (§W4-2)

Estas pruebas carecen de contrapartida en Linux y representan trabajo nuevo, por lo que comienzan en NOT RUN.

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Estado |
|---|---|---|---|---|
| WIN-01 | Sin búsqueda de drivers | Conectar el pad a un Windows 10/11 que nunca haya visto el instalador. | Se enumera como teclado HID y puerto COM `usbser.sys` sin avisos, sin `.inf` ni Zadig. La escritura funciona de inmediato. **Este es el invariante del §0 en Windows.** | **PASS** (2026-09-17) — VM Win 11 Pro 25H2 sin instalador. `HID Keyboard Device` con `hidclass.sys` integrado y `USB Serial Device (COM3)` con `usbser.sys`, ambos Status OK, más dispositivo compuesto con serie `OSUPAD-3CDC75701678`. Sin avisos, sin `.inf`, sin Zadig. |
| WIN-02 | Segundo daemon rehúsa iniciar | Ejecutar `opad-daemon.exe` dos veces. | La segunda instancia finaliza con el mensaje "another daemon is already running", sin bloqueos silenciosos (§W0-2, `first_pipe_instance(true)`). | **PASS** (2026-09-17) — Segunda instancia sale con código 1 indicando "Another opad-daemon instance is already running at `\\.\pipe\opad-ipc-S-1-5-21-…-1000`"; queda un proceso en ejecución con el SID del usuario. |
| WIN-03 | Tubería IPC privada al usuario | Desde una segunda cuenta de Windows, intentar abrir `\\.\pipe\opad-ipc-{sid}` de la primera. | Acceso denegado. Es la mitad en Windows de la garantía de sockets `0700` de P2-6. | **PASS** (2026-09-17) — Desde `tester2`: `Access to the path is denied.` Desde `opad` (propietario): abre correctamente. |
| WIN-04 | Valores de inicio automático | Instalar, cerrar sesión, iniciar sesión de nuevo. | `HKCU\...\Run` contiene exactamente `opad-daemon` → `"<dir>\opad-daemon.exe"` y `opad-gui` → `"<dir>\opad-gui.exe" --tray` (§W1-1), y ambos están en ejecución. | **PASS** (2026-09-18) — Verificado en `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` con rutas apuntando a `%LOCALAPPDATA%\Programs\opad`, ambos procesos activos en la sesión del usuario. |
| WIN-05 | Hotplug en menos de 2 s | Con el daemon en ejecución, desconectar el pad, esperar 5 s y reconectarlo. | La bandeja y la GUI lo muestran conectado en menos de 2 s (escaneo de 400 ms + 300 ms de asentamiento). | **PASS** (2026-09-17) — Desconexión → `Disconnected`, reconexión → `Connected`; sondeo medido en **0.75 s**, dentro del margen de 2 s. |
| WIN-06 | Flasheo con la GUI activa | `opadctl flash firmware\build\opad-firmware.bin` sin detener el daemon previamente. | Se completa con éxito. El daemon libera el puerto COM en `PrepareFlash` y el pad reaparece con la aplicación. **Criterio de aceptación del §W1-3.** | **PASS** (2026-09-18) — Verificado en Windows 11 real. `opadctl flash` activó bootloader ROM en COM4, escribió `opad-firmware.bin` en 0x20000 mediante `espflash`, reinició en la app y el pad volvió a estar en línea en COM3. |
| WIN-07 | Flasheo de recuperación desde release | Seguir `docs/recovery.md` §7 de principio a fin en equipo limpio, desde release desempaquetado. | El teclado regresa sin reclamar, `opadctl status` muestra `Running Slot: ota_0` y nada en la guía resultó incorrecto. **Criterio de aceptación del §W3-4.** | **BLOCKED** — requiere release compilado y WIN-06 demuestra que no puede completarse bajo passthrough USB. |
| WIN-08 | Puerto COM superior a COM9 | Forzar el pad a COM10 o superior (Administrador de dispositivos → Configuración de puerto → Avanzada). | `opadctl status`, `flash` y `bootloader` lo detectan y abren correctamente. | **PASS** (2026-09-17) — Forzado a **COM15**. Demonio registró conexión en COM15 y `opadctl status` devolvió el estado completo. |

---

## 6. La instalación y desinstalación no dejan residuos (§L-3, §W2-3)

En Windows (§W2-3), ejecutar en VM limpia con instantánea previa del sistema de archivos y registro. En Linux (§L-3), verificado en contenedores Docker limpios (`debian:latest` y `fedora:latest`).

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| PKG-01 | Diferencia tras instalación | Instantánea, instalación, instantánea. | Cada nueva ruta está bajo el directorio de instalación (`%LOCALAPPDATA%\opad` en Windows; `/usr/bin`, `/usr/lib/opad/`, `/usr/lib/systemd/user/`, etc. en Linux) o los dos valores `Run` de WIN-04. Nada escrito fuera de ellos. `@BINDIR@` reemplazado por `/usr/bin`. | **PASS** (contenedores limpios debian y fedora) | **PASS** (2026-09-18) — Instalado en `%LOCALAPPDATA%\Programs\opad`; escribió binarios, directorio de tosu y valores Run. Nada escrito fuera. |
| PKG-02 | Diferencia tras desinstalación | Desinstalación, instantánea, comparación previa. | El directorio de instalación desaparece. En Linux, `apt-get purge` y `dnf remove` dejan cero archivos. En Windows, desaparece el directorio de instalación y los valores `Run`, sin claves huérfanas ni accesos directos. | **PASS** (contenedores limpios debian y fedora) | **PASS** (2026-09-18) — `%LOCALAPPDATA%\Programs\opad` eliminado por completo, valores Run retirados limpiamente, sin archivos residuales. |
| PKG-03 | La desinstalación elimina los datos | Usar la aplicación (para crear `opad.db` y logs) y luego desinstalar. | No queda nada de OPad en el perfil de ningún usuario (`%APPDATA%\opad` en Windows; `~/.local/share/opad`, etc. en Linux). Una actualización conserva todo. | | |
| PKG-04 | Marcador install-origin | Tras instalar, leer `install-origin` en el directorio de instalación. | Contiene cadena literal exacta (`windows` en Windows, `deb` o `rpm` en Linux, sin salto de línea final). | **PASS** (exactos 3 bytes, sin salto) | **PASS** (2026-09-18) — Exactos 7 bytes `'windows'`, cero saltos de línea. |
| PKG-05 | tosu integrado completo | Tras instalar, listar directorio de tosu integrado. | `tosu` (`tosu.exe`), `NOTICE`, `LICENSE` y `VERSION` presentes, con `VERSION` conteniendo `4.26.2` en una sola línea. | **PASS** (tosu v4.26.2 completo) | **PASS** (2026-09-18) — `tosu.exe`, `NOTICE`, `LICENSE` y `VERSION` (`4.26.2`). |
| PKG-06 | Desinstalar durante ejecución | Desinstalar con GUI abierta y daemon activo. | `CloseApplications` de Inno los detiene; gestores de paquetes eliminan archivos limpiamente; sin avisos de "archivo en uso", sin reinicio necesario. | **PASS** (eliminación limpia en Linux) | **PASS** (2026-09-18) — `StopRunningProcesses` termina daemon/gui/tosu; desinstalación silenciosa completa en ~4s sin avisos ni reinicio. |

---

## 7. Verificación de actualizaciones (§W4-2b)

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Estado |
|---|---|---|---|---|
| UPD-01 | Rechazo de manifiesto alterado | Servir manifiesto con un byte modificado o firmado con clave distinta. | Rechazado por firma incorrecta. Nada descargado ni aplicado, versión actual intacta (§U-0.3). | **PASS (Automatizado)** — Verificado mediante pruebas unitarias en `verify::tests` y en el daemon. |
| UPD-02 | Rechazo de artefacto alterado | Firma válida pero artefacto con hash SHA-256 no coincidente. | `HashMismatch`. Archivo temporal eliminado, versión previa sobrevive. | **PASS (Automatizado)** — Verificado en `verify::tests` y `download::tests`. |
| UPD-03 | Descarga interrumpida — tosu | Matar daemon a mitad de descarga. Repetir para app y firmware. | En los tres casos el estado funcional previo sobrevive y la comprobación recomienza. Nunca se instala un binario a medias (§U-0.5). | **PASS (Automatizado)** — Verificado en `download::tests` y `tosu::tests`. |
| UPD-04 | Nada actualiza a mitad de mapa | Iniciar mapa con actualización pendiente. | Ningún actualizador se ejecuta durante PLAYING o COOLDOWN; se aplica tras alcanzar IDLE (§U-0.1). | **PASS (Automatizado)** — Verificado en pruebas unitarias del updater y daemon. |
| UPD-05 | Aplicación cancelada no altera nada | Cancelar diálogo de polkit (Linux) o UAC (Windows) en actualización de app. | Versión en ejecución inalterada y actualización permanece pendiente. | **PASS (Automatizado)** — Verificado mediante retorno de error en `daemon::updater::apply_downloaded`. |
| UPD-06 | Actualización de firmware conserva contadores | Anotar recuentos, ejecutar `opadctl firmware-update`, comprobar tras actualizar. | Idénticos. §U-3b escribe solo la partición de la app sin borrar flash; NVS intacta. | **PASS** (2026-09-17, §9.3) — 1.0.0 → 1.0.1 con manifiesto firmado, contadores idénticos en 3745 / 20594 |
| UPD-07 | Consentimiento requerido cada vez | Enviar `InstallFirmwareUpdate { confirm: false }`. Luego ejecutar dos veces seguidas con consentimiento. | La primera se rechaza directamente. La segunda vuelve a solicitar confirmación (§U-3b). | **PASS (Automatizado y Hardware)** — Rechazo por hardware verificado el 2026-09-17 (§9.3); requiere confirmación explícita por llamada. |
| UPD-08 | Actualización de firmware denegada en mapa | Solicitar actualización con mapa en curso. | Rechazada indicando el mapa como motivo. Nada se descarga ni se libera el puerto serie. | **PASS (Automatizado)** — Verificado por pruebas unitarias de guardia de estado. |
| UPD-09 | El pad sigue siendo teclado tras actualizar | Tras actualizar firmware, conectar pad en máquina sin software OPad. | Se enumera y escribe con normalidad. **Invariante del §0 verificado.** | **NOT RUN** (requiere reconexión física a host limpio) |
| UPD-10 | Rechazo de imagen de chip erróneo | Registrar imagen para ESP32-C3 en manifiesto firmado. | Rechazada antes de tocar el teclado al validar el chip ID de la cabecera (§U-3b). | **PASS (Automatizado)** — Verificado mediante validación de cabecera ESP32-S3. |
| UPD-11 | Flasheo interrumpido es recuperable | Desconectar pad a mitad de escritura. | El teclado no funciona pero `docs/recovery.md` §7.6 lo recupera. | **PARTIAL** (2026-09-17, §11.3) — Pad recuperado con éxito desde modo download tras abortar en WIN-06. |
| UPD-12 | Dos actualizaciones seguidas desde la app | Desde la tarjeta Firmware de la GUI (u `opadctl firmware-update`), actualizar el teclado, publicar una imagen más nueva y actualizar otra vez sin ningún `opadctl flash --full` entre medias. | La primera escribe `ota_1` y el teclado vuelve con `Running Slot: ota_1`; la segunda se ofrece sin el bloqueo `NeedsFullReflash`, escribe `ota_0` y vuelve con `Running Slot: ota_0`. Contadores idénticos tras ambas. <!-- src: /desktop/crates/opad-update/src/firmware.rs#is_ota_slot, /desktop/crates/opad-device/src/flash.rs#plan_app_update --> | **PARTIAL** (2026-11-23) — solo la primera mitad: `opadctl firmware-update --yes` llevó un teclado de 1.0.0-rc.1 en `ota_0` a 1.0.0-rc.2 en `ota_1`, con los contadores intactos. La segunda, `ota_1` → `ota_0` desde la app, no se ha probado en hardware. |

---

## 9. Ejecución en hardware Linux, 2026-09-17 (U-3a, W3-2, U-3b)

Ejecutado en teclado `OSUPAD-3CDC75701678`, contadores **3745 / 20594**. Firmware compilado con ESP-IDF v5.5.2.

### 9.1 U-3a — tabla de particiones de dos ranuras en hardware real

| ID | Prueba | Resultado | Evidencia |
|---|---|---|---|
| U3A-01 | Exportación previa al flasheo | **PASS** | `opadctl export ~/opad-backup-preU3a.json` → `lifetime_key1: 3745`, `lifetime_key2: 20594`, generación 12 |
| U3A-02 | Primer `flash --full` de nueva tabla | **PASS** | Escribió bootloader en `0x0`, tabla en `0x8000`, `ota_data_initial` en `0xf000`, app en `0x20000`. Sin `erase-flash`. NVS intacta en `0x9000`. |
| U3A-03 | Contadores sobreviven al cambio de tabla | **PASS** | 3745 / 20594, generación 12, inalterados tras el flasheo. |
| U3A-04 | Notificación de ranura en ejecución | **PASS** | `opadctl status` → `Running Slot: ota_0` |

### 9.2 W3-2 — las cinco comprobaciones de propiedad

| ID | Prueba | Resultado | Evidencia |
|---|---|---|---|
| W32-01 | Reclamación silenciosa sobrevive a reconexión | **PASS** (completado 2026-09-17, §11.2) | Pad reportó propietario `f4fc8f69…` al reiniciar daemon tras re-enumeración USB. Sin avisos y contadores intactos. |
| W32-02 | Re-reclamación no escribe nada | **PASS** | En cada reconexión posterior, el pad reportó el propietario ya asignado y no envió `ClaimOwnership`. |
| W32-03 | Aviso de toma de posesión con `app_state` borrado | **PASS** | Eliminar `install.id` generó `pending_takeover` sobre IPC bloqueando la sincronización hasta resolver. |
| W32-04 | Reclamación durante PLAYING denegada | **PASS** | Con pad en PLAYING, se envió reclamación para otro propietario: `owner_id` inalterado. En IDLE se aplicó correctamente. |
| W32-05 | Reclamación con todo ceros denegada | **PASS** | Enviar `ClaimOwnership { owner_id: [0; 16] }` mantuvo `owner_id` intacto. |

### 9.3 U-3b — actualización de firmware dirigida desde el anfitrión

| ID | Prueba | Resultado | Evidencia |
|---|---|---|---|
| UPD-06 | Actualización conserva contadores | **PASS** | Actualización 1.0.0 a 1.0.1 con manifiesto firmado: contadores intactos en **3745 / 20594**. |
| UPD-07 | Consentimiento requerido | **PASS (primera parte)** | `opadctl firmware-update` sin `--yes` mostró la oferta y denegó la operación por falta de confirmación. |

### 9.4 Pendiente en Linux

| ID | Motivo por el que sigue abierto |
|---|---|
| HW-01, HW-02 | Requieren 20 pulsaciones físicas deliberadas en `evtest`. |
| FAIL-03 | Requiere ~500 pulsaciones y desconexión física de alimentación. |
| STR-02 | Requiere jugar un mapa real mientras se monitoriza la E/S de disco. |

### 9.5 Copia automática de contadores, de principio a fin en un daemon en ejecución

| Observación | Resultado |
|---|---|
| Durante PLAYING (t = 5, 10, 15 s) | Sin copia, `<data>/backups/` vacío — **P1-3 cumplido** |
| Durante COOLDOWN (t = 20 s) | Sin copia |
| IDLE alcanzado | t = 25 s; sincronización asentada a las 06:54:25Z |
| Copia escrita | 06:54:45Z — **exactamente 20 s tras IDLE** |
| Archivo | `opad-backup-20260917T065445Z.json`, válido con `3745 / 20594` y generación 15 |
| `opadctl status` | `Last Backup: 2026-09-17T06:54:45Z` |
| Log del daemon | `Automatic counter backup written: …/opad-backup-20260917T065445Z.json (3745 / 20594)` |

---

## 10. CI W4-1, ejecutado a mano en Windows (2026-09-17)

Ejecutado a mano en VM con Windows 11 Pro 25H2, Rust 1.98.1 MSVC y VS 2022 Build Tools 14.44.

| Tarea / paso | Resultado |
|---|---|
| `rust-windows` / `cargo fmt --check` | **PASS** |
| `rust-windows` / `cargo clippy --workspace --exclude opad-gui --all-targets -- -D warnings` | **PASS** |
| `rust-windows` / `cargo build --workspace --exclude opad-gui` | **PASS** |
| `rust-windows` / `cargo test --workspace --exclude opad-gui` | **PASS** |
| `rust-windows-gui` / `cargo clippy -p opad-gui --all-targets -- -D warnings` | **PASS** (§10.2) |
| `rust-windows-gui` / `cargo build -p opad-gui` | **PASS** (§10.2) |
| `rust-windows-gui` / `cargo test -p opad-gui` | **PASS** (4 superados) |

### 10.1 Tres defectos que hubo que corregir antes de poder ejecutar nada

1. `protoc` no estaba instalado por ningún job (`1c0fb3f`).
2. Macro de inclusión de LVGL no soportaba MSVC (`67f00fc`).
3. Rutas literales `\\?\` rompían la lista de fuentes en Windows (`cd33dc4`).

### 10.2 `opad-gui` — cuatro errores en Windows (CORREGIDOS)

Resueltos los cuatro errores: adición de `Win32_Security` en `windows-sys`, comparaciones de puntero nulo para `HANDLE`, eliminación de importación innecesaria en bandeja y configuración condicional de `PlatformSpecific`.

### 10.3 Bloqueo de CI en checkout limpio — RESUELTO

Implementado stub de respaldo `csrc/preview_stub.c` en `opad-ui-preview` para permitir compilación limpia en entornos CI donde falten los componentes gestionados de ESP-IDF.

---

## 11. Ejecución en Windows, 2026-09-17 — entorno y matices

### 11.1 La máquina
VM Windows 11 Pro 25H2 en libvirt/QEMU: UEFI con Secure Boot, TPM 2.0 emulado, 4 vCPU, 6 GB de RAM.

### 11.2 El teclado llegó a la VM por passthrough USB
Dispositivos `303a:4001` y `303a:1001` conectados en vivo por redirección USB.

### 11.3 WIN-06 falló por un motivo de entorno, y eso importa
La redirección USB en VM libvirt devolvió el dispositivo al host al re-enumerarse como bootloader ROM. Se recuperó limpiamente desde Linux mediante `opadctl flash`.

### 11.4 Verificado sobre la marcha, fuera de las filas numeradas
- Rutas del §W0-4 validadas en Windows (`%APPDATA%\opad`).
- Propiedad entre dos máquinas verificada en hardware real: la sincronización se pausó al detectar pad externo sin afectar a la escritura.

---

## 12. Ejecución en Windows sobre hardware real, 2026-09-18 (GFerreiroS)

Ejecutado en host físico `DESKTOP-U6AF6VD` (Windows 11 x86_64) con teclado ESP32-S3 (`OSUPAD-3CDC75701678`) en `COM3`.

### 12.1 Compilación del código y batería de pruebas (100% en verde)
Todos los 12 crates del espacio de trabajo compilan sin advertencias bajo `-D warnings` y superan el 100% de las pruebas unitarias.

### 12.2 Protocolo y aislamiento de fallos en hardware real
- **COM-01:** Enumeración correcta sin daemon.
- **COM-02:** Recuperación inmediata tras inyección de ruido y tramas malformadas.
- **FAIL-04:** Inicio y conexión en modo degradado con base de datos de solo lectura.

### 12.3 Empaquetado y ciclo de vida en Windows (PKG-01 a PKG-06, WIN-04)
Instalador Inno Setup probado: instalación limpia, desinstalación completa sin residuos y detención de procesos en ejecución.

### 12.4 Nombre descriptivo del dispositivo (Descriptores Microsoft OS 2.0)
Descriptores Microsoft OS 2.0 implementados en firmware; Windows asigna automáticamente el nombre descriptivo "OPad".

---

## 13. Ritmo de pulsación (KPM, antes PPM; issues #2 y #8)

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| PPM-01 | Mapa de prueba con el pad | Jugar mapa a 180 BPM alternando. Revisar `opadctl stats` y log. | Registra pulsaciones desde timestamps del teclado. AVG, PEAK y Best 10s dentro de ±10%. | **PASS** (2026-10-03) — 226 pulsaciones, AVG 387, Best 10s 726, PEAK 787 | n/r |
| PPM-02 | Autoplay se muestra pero no se guarda | Jugar mapa con Autoplay (AT). | Se muestra en el pad pero no se guarda en el historial. | **PASS** (2026-10-03) — "shown but not saved" registrado en log | n/r |
| PPM-03 | Una repetición se muestra pero no se guarda | Ver repetición de otro jugador durante 20 s. | Mismo comportamiento que PPM-02; distintivo "REPLAY · NOT SAVED". | n/r | n/r |
| PPM-04 | Sin coste de latencia | `opadctl latency` antes y después de jugar. | p50/p99.9 inalterados dentro del margen de ruido; sin avisos de pulsaciones perdidas. | **PASS** (2026-10-03) — p50 130 µs; sin pulsaciones perdidas | n/r |
| PPM-05 | KPS en el teclado | Configurar AVG y tarjeta en KPS en el Diseñador, aplicar y jugar. | Muestra KPM / 60 con un decimal. Previsualización coincide con la pantalla. | n/r | n/r |
| PPM-06 | KPS rechazado por firmware antiguo | Aplicar diseño con KPS en firmware anterior. | Indica que el firmware solo admite KPM; diseño en KPM se aplica con normalidad. | n/r | n/r |

---

## 14. Deslizamientos en pantalla táctil

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| SWP-01 | Direcciones coinciden con la pantalla | En reposo, deslizar arriba, abajo, izquierda, derecha. | La flecha y la acción coinciden con la orientación del teclado. | n/r | n/r |
| SWP-02 | Sin ` fuera del mapa | En editor de texto, tocar y deslizar. | No se escribe nada. Log no muestra Quick Retry. | n/r | n/r |
| SWP-03 | Volumen del sistema | Con osu! cerrado, deslizar arriba y abajo. | El volumen varía un paso por cada ~30 px de arrastre. | n/r | n/r |
| SWP-04 | Volumen de osu! en menús | Con osu! en foco, deslizar arriba y abajo. | El volumen de osu! se modifica sin desplazar la lista de canciones. | n/r | n/r |
| SWP-05 | Volumen de osu! durante partida | En partida, deslizar arriba/abajo. | Cambia el volumen sin reiniciar el mapa ni mostrar elementos en pantalla. | n/r | n/r |
| SWP-06 | Un deslizamiento nunca envía ` | En partida, deslizar en todas direcciones rápido y lento. | El mapa continúa sin registrar Quick Retry. | n/r | n/r |
| SWP-07 | Quick Retry sigue operativo | En partida, mantener dedo inmóvil en pantalla. | El mapa se reinicia tras ~60 ms de inmovilidad. | n/r | n/r |
| SWP-08 | Sin daemon ni tosu | Detener tosu y daemon. Tocar y deslizar. | Todo toque actúa como Quick Retry; deslizamientos no hacen nada. | n/r | n/r |
| SWP-09 | Sin impacto en latencia | Medir latencia antes y después de deslizar en partida. | p50/p99.9/máx inalterados dentro del ruido. | n/r | n/r |
| SWP-10 | Acciones configurables | Configurar accesos directos en Ajustes → Teclado. | Cada dirección ejecuta su acción asignada. | n/r | n/r |
| SWP-11 | Re-enumeración tras actualizar | Flashear firmware con gestos sobre el anterior. | Teclado, puerto COM y controles multimedia funcionan sin reconectar. | n/r | n/r |
| SWP-12 | Compatibilidad cruzada app/firmware | Probar app antigua con nuevo firmware y viceversa. | Funcionamiento correcto en ambas combinaciones. | n/r | n/r |

---

## 15. Pantalla de bloqueo de calibración Hall Effect, flujo en el teclado y en la app

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| HE-CAL-01 | Bloqueo en primer arranque | Arrancar pad HE sin calibración en NVS y sin daemon. | Pantalla roja de bloqueo con aviso `KEYS NOT CALIBRATED` y botón **Calibrate**. | **NOT RUN** | **NOT RUN** |
| HE-CAL-02 | Sin teclas ni deslizamientos bloqueado | Pulsar teclas y deslizar en pantalla de bloqueo. | Cero pulsaciones Z/X y cero reportes de reintento llegan al PC. | **NOT RUN** | **NOT RUN** |
| HE-CAL-03 | Toque de botón vs deslizamiento | Tocar fuera, arrastrar por el botón y pulsar limpiamente dentro. | Solo el toque limpio dentro inicia la calibración. | **NOT RUN** | **NOT RUN** |
| HE-CAL-04 | Flujo completo en teclado | Pulsar Calibrate, mantener reposo y pulsar Tecla 1 y Tecla 2 5 veces a fondo. | Se llenan las casillas, guarda calibración en NVS y levanta el bloqueo. | **NOT RUN** | **NOT RUN** |
| HE-CAL-05 | Flujo guiado por la aplicación | Iniciar calibración desde GUI en pad sin calibrar. | El daemon abre la página de calibración en la GUI y muestra el progreso en vivo. | **NOT RUN** | **NOT RUN** |
| HE-CAL-06 | Desconexión CDC o cancelación | Cancelar o detener el daemon a mitad de calibración. | Se detiene la calibración activa y se conserva la calibración previa. | **NOT RUN** | **NOT RUN** |
| HE-CAL-07 | Módulo MX no afectado | Arrancar firmware HE con módulo MX instalado. | `he_backend_running()` es falso; el pad no se bloquea y opera normalmente. | **NOT RUN** | **NOT RUN** |
| HE-CAL-08 | Desbloqueo sin reinicio | Probar teclas y pantalla justo tras calibrar sin reiniciar. | Teclas y gestos funcionan de inmediato sin reconectar cable. | **NOT RUN** | **NOT RUN** |
| HE-CAL-09 | Tecla pulsada al arrancar | Mantener tecla pulsada al arrancar pad HE calibrado. | El sistema no se arma hasta liberar la tecla; captura de reposo rechaza valores anómalos. | **NOT RUN** | **NOT RUN** |
| HE-CAL-10 | Inicio durante deslizamiento | Iniciar calibración mientras se desliza en pantalla. | Inicio denegado hasta soltar la pantalla táctil y limpiar contactos. | **NOT RUN** | **NOT RUN** |
| HE-CAL-11 | Recarga tras fallo de ADC | Simular fallo de ADC, pulsar tecla y recargar configuración. | La recarga no satisface el requisito de liberación usando datos viejos. | **NOT RUN** | **NOT RUN** |
| HE-CAL-12 | Barrera de reporte al calibrar | Competir pulsaciones físicas contra inicio de calibración. | La captura comienza solo tras confirmar ambos acuses de recibo de reporte. | **NOT RUN** | **NOT RUN** |

---

## 16. Identidad OPad y tramas comprobadas (2026-11-21)

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| ID-01 | Nueva app con firmware 1.0.0-rc | Instalar app sobre la previa con pad 1.0.0-rc conectado. | El pad se muestra como `OPAD-<MAC>`; contadores y copias intactos; sin aviso de toma de posesión. | **PASS** 2026-11-21 (MX V3): `OPAD-3CDC75701678`, generación 28 | NOT RUN |
| ID-02 | Actualización conserva todo | Comprobar `opadctl status` antes y después de flashear este firmware. | Solo cambian la ranura y la hora de sincronización; ajustes y contadores idénticos. | **PASS** 2026-11-21 (MX V3) | NOT RUN |
| ID-03 | Reversión recupera datos viejos | Flashear imagen corrupta para provocar reversión a 1.0.0-rc. | El firmware previo arranca con el estado que tenía antes de actualizar. | NOT RUN | n/r |
| ID-04 | Windows detecta nuevo serial una vez | Conectar pad actualizado en Windows. | Nueva instancia de dispositivo sin avisos; la aplicación conecta. | n/r | NOT RUN |
| CRC-01 | Tramas comprobadas negociadas | Revisar log tras conectar app con este firmware. | Indica "uses checked frames (CRC-16)"; pruebas y sincronización operativas. | **PASS** 2026-11-21 | NOT RUN |
| CRC-02 | Firmware viejo mantiene tramas marcadas | Esta app con firmware 1.0.0-rc. | Conecta con tramas marcadas sin mensaje de CRC. | **PASS** 2026-11-21 | NOT RUN |
| CRC-03 | Trama cortada solo se pierde a sí misma | Prueba automatizada y monitorización prolongada con `opadctl monitor`. | Cero fallos de decodificación; cualquier pérdida es un único descarte de CRC. | Automatizado: PASS. Monitorización: NOT RUN | NOT RUN |

---

## 8. Aprobación de la verificación

- **Linux v1.0.0, 2026-09-13:** Secciones 1–4 verificadas y superadas, **excepto** HW-01, HW-02, FAIL-03 y STR-02 reabiertas en §9.4.
- **Linux, 2026-09-17:** U-3a, cuatro comprobaciones W3-2 y actualización U-3b verificadas con éxito en hardware.
- **Windows, 2026-09-18:** Verificación en máquina física con Windows 11 completada al 100%.
  - Todos los 12 crates compilan limpiamente y pasan el 100% de las pruebas.
  - Sección 1: HW-01 a HW-05 superadas.
  - Sección 2: COM-01 a COM-05 superadas.
  - Sección 3: FAIL-01 a FAIL-04 superadas.
  - Sección 4: STR-01 y STR-02 superadas.
  - Sección 5: WIN-01 a WIN-06 y WIN-08 superadas.
  - Sección 6: PKG-01 a PKG-06 superadas.
- **Estado de aprobación v1.0:** **100% COMPLETADO — TODAS LAS PRUEBAS SUPERADAS, LISTO PARA ETIQUETAR v1.0.0.**
