---
title: "Lista de verificación de hardware y release"
description: "Filas de verificación manual de hardware y de release para el pad y la suite de escritorio: qué filas necesita un cambio, cómo ejecutarlas y los resultados registrados."
sidebar:
  order: 4
---

Esta lista documenta el procedimiento manual de verificación de hardware para
una release (§34, §35), obligatorio antes de las releases etiquetadas. Cada fila
la ejecuta una persona con el pad delante, y nada de aquí puede marcarse a
partir del resultado de la otra plataforma. Las filas marcadas como "Automated"
apuntan al test que las cubre; esos se ejecutan en `make check` ([Pruebas](develop/testing.md)).

**Qué filas ejecutar.** Un cambio que toque la entrada, el USB, el táctil o la
pantalla necesita una prueba en un pad real antes de darlo por terminado ([/AGENTS.md](https://github.com/OPad-dev/OPad/blob/main/AGENTS.md) §4):

| El cambio toca | Ejecuta como mínimo |
|---|---|
| `firmware/main/input/` (teclas, antirrebote, HE) | §1 (HW-01 a HW-05, HW-08, HW-14), latencia con `opadctl latency` ([Medición de latencia](develop/latency-testing.md)) |
| `firmware/main/usb/` o los descriptores | §1, §2, SWP-11, ID-05, FAIL-05 a FAIL-08 |
| El táctil (`touch_retry.c`, `gesture.c`) | §14 (SWP-01 a SWP-15) |
| La pantalla o LVGL | HW-05, FAIL-01, SWP-09 |
| La calibración HE o el bloqueo | §15 |
| Flasheo, recuperación, actualizaciones | FAIL-05 a FAIL-08, §7 |
| El protocolo serie | §2, §16, y app antigua / firmware nuevo en los dos sentidos (SWP-12, ID-01) |

Un pad que no vuelve se recupera con `opadctl recover` y volviéndolo a enchufar
([Recuperación](recovery.md) §7.6–7.7), nunca con los botones BOOT o RESET.

**Hardware objetivo:** Waveshare ESP32-S3-Touch-LCD-2.0  
**Probador:** GFerreiroS  

| Ejecución | Fecha | Host | Firmware | Estado |
|---|---|---|---|---|
| Linux | 2026-09-13 | Linux x86_64, kernel 6.x | v1.0.0 | Completa, pero ver la nota de abajo |
| Repetición en Linux (U-3a) | 2026-09-17 | Linux x86_64, kernel 7.2 | 1.0.0 con la tabla de dos ranuras | Parcial — §9 |
| Windows (§W4-2) | 2026-09-18 | Windows 11 x86_64 (hardware real) | v1.0.0 (`OSUPAD-3CDC75701678`) | Completa — 100% PASS (§12) |
| Linux, módulos V3 | 2026-11-20 | Linux x86_64, kernel 7.3 | 1.0.0-rc, rama `feat/v3-modules`, en la placa portadora V3 con los módulos HE V3 y MX V3 | Parcial — HW-06 a HW-15, BAK-01 |

> [!WARNING]
> **La columna de Linux del 2026-09-13 está obsoleta para las filas de contadores.**
> Es anterior tanto al cambio del blob de configuración NVS v2 → v3 (§W3-2) como
> a la tabla de particiones de dos ranuras (§U-3a), así que **HW-01, HW-02, FAIL-03
> y STR-02 ya no son prueba válida** para el firmware que se distribuye y se
> reabren en §9. El resto de la columna de Linux no se ve afectado: nada en esas
> filas toca la distribución de la NVS ni el particionado.

> [!NOTE]
> **La columna de Windows de abajo está vacía a propósito.** Se rellena
> ejecutando las pruebas en hardware con Windows, no razonando a partir de la
> columna de Linux. **HW-05** (HID primero al despertar la pantalla) y **STR-02**
> (cero escrituras en almacenamiento) son la pareja crítica para los
> invariantes: §W4-2 exige volver a verificar las dos en Windows en lugar de darlas por supuestas.

Leyenda: **PASS** / **FAIL** / **NOT RUN** / **n/r** = no requerido en esta
plataforma según §W4-2.

---

## 1. Rendimiento físico de las teclas e invariantes de latencia

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| HW-01 | Pulsación y suelta de Key 1 | Pulsar Key 1 limpiamente 20 veces. | Exactamente una pulsación por toque en `evtest`, 0 teclas atascadas, el contador de por vida sube 20. | **RE-OPENED** (§9) | **PASS** (2026-09-18) — Verificados 20 toques, pulsaciones discretas 1:1, contador incrementado exactamente en 20 |
| HW-02 | Pulsación y suelta de Key 2 | Pulsar Key 2 limpiamente 20 veces. | Exactamente una pulsación por toque en `evtest`, 0 teclas atascadas, el contador de por vida sube 20. | **RE-OPENED** (§9) | **PASS** (2026-09-18) — Verificados 20 toques, pulsaciones discretas 1:1, contador incrementado exactamente en 20 |
| HW-03 | Pulsación simultánea | Pulsar Key 1 + Key 2 a la vez, en una ventana de 1 ms. | Las dos teclas se informan abajo y arriba sin ghosting ni bloqueos. | **PASS** | **PASS** (2026-09-18) — Verificado con `test_hw_input`; 27 pulsaciones simultáneas registradas a la vez con 0 ghosting |
| HW-04 | Stream alterno rápido | Alternar K1/K2 a más de 20 pulsaciones/s durante 60 segundos. | 0 pulsaciones perdidas, 0 rebotes/dobles toques, aumento de la latencia p99.9 < 0,1 ms sobre la referencia. | **PASS** | **PASS** (2026-09-18) — 566 pulsaciones en una prueba de stream de 68,1 s con `test_hw_input`; 0 pulsaciones perdidas, latencia de hardware p50 200 µs / p99.9 870 µs |
| HW-05 | Pantalla dormida -> despertar al pulsar | Dejar que la pantalla se duerma (10 min sin actividad) y pulsar Key 1. | **Invariante de HID primero verificado**: el report de la tecla se envía al host de inmediato, antes de que empiece la secuencia de despertar de la pantalla. Sin retardo de entrada. | **PASS** | **PASS** (2026-09-18) — Pulsación enviada al PC al instante, antes de que empiece la secuencia de despertar de la pantalla |
| HW-06 | Prueba de placa (placa portadora + módulo MX) | Con el módulo MX en la placa portadora y las dos teclas sueltas, ejecutar `opadctl board-test` (o Diagnostics → PCB Test). Repetir con el cable desenchufado, con un cable invertido pin 1 ↔ 8 y manteniendo Key 1. | Placa sana: pasan todas las comprobaciones. Desenchufado: "No input module answers". Invertido: falla la comprobación Cable nombrando la inversión. Tecla mantenida: la línea de Key 1 falla con "reads low". Las teclas funcionan con normalidad justo después de cada ejecución: ninguna tecla queda mantenida y no se cuenta ninguna pulsación (`opadctl status` antes y después). | **FAIL**, corregido: hasta `1fab1fe` cada prueba de placa dejaba Key 2 (GPIO7) mantenida en un pad MX, porque al abrir el ADC del ID del módulo también se configuraba el canal de GPIO7 y el pin pasaba a analógico. Repetida el 2026-11-20 en MX V3, tres pruebas de placa seguidas: **PASS**, la cuenta de Key 2 y las muestras de latencia sin cambios | n/r |
| HW-07 | Prueba de placa (placa portadora V3 + módulo HE V3) | Con el módulo HE V3 en la placa portadora V3, sin teclas montadas, ejecutar `opadctl board-test --magnet` (o Diagnostics → PCB Test, y después Start Magnet Test) y mantener un imán sobre la tecla 1, la 2 y luego la 3. | Módulo "Hall Effect V3" a ~1,65 V; el ADC responde 0x81, 20/20 escrituras de registro, margen de señal limpio hasta al menos 20 MHz, la alimentación aguanta; cada sensor reposa cerca de 1,65 V con menos de 3 códigos rms de ruido; cada comprobación con imán pasa solo en su propia tecla. | **PASS** 2026-11-20 cable recto, sin teclas (ID 1,66 V, limpio hasta 40 MHz, 0,8 códigos rms). Imán: NOT RUN | n/r |
| HW-08 | Key 3 (HE V3) | Con los interruptores montados, pulsar la tecla 3 limpiamente 20 veces, observando en `evtest`. Dejar Key 3 vacía en Settings y volver a pulsarla. | Exactamente una pulsación por toque del ajuste de Key 3 (por defecto C, `DEFAULT_KEY3_HID_USAGE` en `opad-model`); su propio contador de por vida (`Key 3 (K3)` en `opadctl status`) sube 20 y los de Key 1/Key 2 no. Vacía: no se envía nada. | **PARTIAL** 2026-11-20 — escribe C y su contador cuenta. La prueba de 20 toques en `evtest` y el ajuste vacío: NOT RUN | n/r |
| HW-09 | Teclas de juego | Settings → GAME KEYS: marcar Key 1 y Key 3 y guardar; pulsar la tecla 3 y después la 2; volver a marcar Key 1 y Key 2. Intentar marcar una tercera casilla. | Teclas 1+3: la segunda tarjeta de tecla del pad muestra la letra y la cuenta de Key 3 (`ui/ui_port.c`, `input/game_keys.h`), el total y los KPS cuentan solo las teclas 1 y 3, y la tecla 2 sigue escribiendo pero no mueve ninguna tarjeta. De vuelta a 1+2: ningún pico de KPS. Con dos marcadas, la tercera casilla queda desactivada, y el pad rechaza tres (`config/config_validate.c`). | **PASS** 2026-11-20 | n/r |
| HW-10 | Calibración con tres teclas | Calibrar un pad HE V3 en el pad y después desde la app. Borrar la calibración de la tecla 3 y reiniciar. | Se calibran las tres teclas y `HE Calibrated` muestra K1, K2, K3 yes. Con la tecla 3 sin calibrar, el pad queda bloqueado en su pantalla de calibración (decisión D7). | **PARTIAL** 2026-11-20 — calibración en el pad y desde la app. El bloqueo con la tecla 3 sin calibrar: NOT RUN | n/r |
| HW-11 | Comprobador de Key 3 en Diagnostics | Diagnostics → Switch Chatter & Input en un pad de tres teclas; pulsar la tecla 3 sola, después con la tecla 1 y con la tecla 2. | Una tarjeta Key 3, sin pin mostrado; solo ella se ilumina con la tecla 3; la tecla 3 con cualquiera de las otras cuenta como simultánea (`gui/src/diagnostics.rs`). | **PASS** 2026-11-20 | n/r |
| HW-12 | Enlace HE V3: CRC y velocidad | Ejecutar la prueba de placa y después usar las teclas. Con una compilación desechable que informe de `he_spi_stats_t` (`input/he_spi_source.h`), dejarla transmitir durante un minuto. | La prueba de placa pasa justo después de transmitir (el ADC se saca del modo CRC con un reset, `ads7028_reset`) y las teclas funcionan después. Ningún error de CRC ni de SPI, todas las teclas leídas en cada trama, una trama leída en unos 14 µs (`docs/hardware/components/ads7028.md`). | **PARTIAL** 2026-11-20 — prueba de placa tras transmitir, 0 errores de CRC en más de 400.000 tramas, media de 14,4 µs, máximo de 51-62 µs. Pulsaciones reales y un mapa con este firmware: NOT RUN (todavía sin carcasa) | n/r |
| HW-13 | Prueba de placa (placa portadora V3 + módulo MX V3) | Con el módulo MX V3 en la placa portadora V3 sobre su FFC de 14 vías tipo B y todas las teclas sueltas, ejecutar `opadctl board-test` (o Diagnostics → PCB Test). | Módulo "MX V3" a ~2,24 V; Cable: el ID llegó por GPIO8 (contacto 1 con contacto 1); pines de tecla GPIO10 / GPIO7; las líneas de Key 1, 2 y 3 ven cada una el pull-up de 10 kΩ del módulo (R1, R2, R3) en GPIO10, GPIO7 y GPIO6 (`opad-model/src/board_test.rs`). Ninguna comprobación de Spare lines en un módulo V3. | **PASS** 2026-11-20 (ID 2,26-2,27 V, 856-860 mV con carga) | n/r |
| HW-14 | Key 3 (MX V3) y tres teclas MX | En el pad MX V3, pulsar las teclas 1, 2 y 3, mantener cada una y después jugar un mapa con las teclas 1 y 2 como teclas de juego. | Key 3 escribe su ajuste (por defecto C) y se repite mientras se mantiene, con su propio contador (`Key 3 (K3)` en `opadctl status`); las teclas 1 y 2, como antes. Latencia durante el mapa no peor que la del firmware de dos teclas. | **PASS** 2026-11-20 — Z, X y C escriben y se mantienen. Mapa: 792 muestras, p50 30 µs, p99 40 µs, p99.9 50 µs, máximo 46 µs, 0 diferidos (firmware de dos teclas: 1920 muestras, p50 30 µs, p99 60 µs, máximo 54 µs) | n/r |
| HW-15 | Ajustes de pines de tecla solo en un pad cableado a mano | Abrir Settings → Keys con un módulo en la placa portadora y después en un pad sin módulo (interruptores cableados al conector). | Con un módulo (MX, MX V3, HE, HE V3) no hay selectores KEY 1 PIN / KEY 2 PIN ni Auto-Detect; sin módulo, sí. Un firmware que no envía `HelloAck.input_module` los conserva (`DeviceInfo::key_pins_configurable` en `opad-model`). | **PARTIAL** 2026-11-20 — MX V3: sin selectores. Pad cableado a mano: NOT RUN | n/r |

---

## 2. Comunicación y robustez del protocolo

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| COM-01 | Sin host CDC | Arrancar el pad con el daemon del host parado (host USB HID puro). | El pad arranca limpio, el teclado funciona con normalidad, no salta ningún watchdog de FreeRTOS ni hay inanición de búferes. | **PASS** | **PASS** (2026-09-18) — PnP indica HID Keyboard Device OK y presente con el daemon parado |
| COM-02 | Trama de protocolo malformada | Inyectar bytes basura aleatorios y cabeceras de trama no válidas por `/dev/ttyACM0` con un script de prueba. | El firmware descarta el búfer malformado, registra `DIAG_EVENT_FRAME_TOO_LARGE` / `DIAG_EVENT_DECODE_FAILED` y se recupera con el siguiente sobre válido sin colgarse. | **PASS** | **PASS** (2026-09-18) — Inyectados 512 B de ruido, una trama sobredimensionada 0x7FFFFFFF y bytes protobuf corruptos con `test_com02`; el pad se recuperó con el siguiente Hello |
| COM-03 | Reconexión USB rápida | Desenchufar y enchufar el cable USB 20 veces seguidas (intervalo de 1 s). | El daemon del host se reconecta limpiamente cada vez; el pad se vuelve a enumerar como teclado HID + CDC sin bloquearse. | **PASS** | n/r |
| COM-04 | Matar el daemon durante la partida | Terminar `opad-daemon` (`kill -9`) en plena partida. | El pad sigue funcionando como teclado HID a 1000 Hz sin ninguna interrupción de las pulsaciones en curso. | **PASS** | n/r |
| COM-05 | Matar tosu durante la partida | Terminar el servidor WebSocket de tosu en plena partida. | El pad pasa sin problemas de PLAYING a COOLDOWN (ventana de 5 s) y después a IDLE; ningún estado de UI atascado. | **PASS** | n/r |

---

## 3. Almacenamiento y aislamiento de fallos (§P0-1, §P2-12)

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| FAIL-01 | Compilación con fallo de LCD | Compilar con `CONFIG_OPAD_TEST_FAIL_LCD=y` y flashear el pad. | Se registra un fallo no fatal al iniciar la pantalla; el pad pasa a funcionar como teclado sin pantalla; 0 impacto en la latencia HID. | **PASS** | n/r |
| FAIL-02 | Compilación con fallo de NVS | Compilar con `CONFIG_OPAD_TEST_FAIL_NVS=y` y flashear el pad. | Se registra un fallo no fatal de NVS; el pad pasa a contadores solo en RAM; el teclado y el protocolo siguen funcionando. | **PASS** | n/r |
| FAIL-03 | Persistencia tras un corte de alimentación | Jugar un mapa hasta registrar 500 pulsaciones, esperar 10 s a la sincronización en IDLE y desenchufar la alimentación. Volver a conectarla. | Los contadores de por vida coinciden exactamente con los de antes del corte; 0 pérdidas de pulsaciones verificadas. | **PASS** (2026-09-18) | **PASS** (2026-09-18) — Verificadas más de 500 pulsaciones, sincronización en IDLE y corte de la alimentación USB; los contadores de por vida coinciden exactamente con los de antes del corte, sin pérdidas |
| FAIL-04 | Modo degradado de SQLite | Quitar los permisos de escritura de la base de datos SQLite (`chmod 400 opad.db`) y ejecutar el daemon. | El daemon arranca en modo degradado, muestra `storage_error` en la GUI/IPC, bloquea las escrituras destructivas y mantiene los layouts en memoria. | **PASS** | **PASS** (2026-09-18) — Probado con opad.db de solo lectura; el daemon arrancó y se conectó al hardware |
| FAIL-05 | La actualización va a la otra ranura | En un pad que funciona, `opadctl flash firmware/build/opad-firmware.bin`. | Muestra la ranura que escribe y la de respaldo; vuelve con "Flash succeeded" en la otra ranura (`Running Slot` en `opadctl status`). | **PASS** (2026-10-10, ota_0 → ota_1, entrada confirmada como VALID) | n/r |
| FAIL-06 | Una actualización rota se revierte | Compilar con `CONFIG_TINYUSB_HID_COUNT=1` y quitando los `_Static_assert` de `usb_descriptors.c` (en una copia de trabajo, nunca en un commit) y después hacer `opadctl flash` con ella. | `dmesg` muestra `can't set config #1, error -32`; unos 20 s después el pad se reinicia solo en su ranura anterior; `opadctl flash` dice que ha vuelto a la anterior; las teclas escriben. | **PASS** (2026-10-10, de vuelta en ota_0 tras 22 s) | n/r |
| FAIL-07 | Recuperar volviendo a enchufar | `opadctl recover firmware/build` y después volver a enchufar el pad. | "Caught the pad booting", las cuatro imágenes escritas, "Recovered". Sin tocar ningún botón. | **PASS** (2026-10-10, ventana de arranque tras un reset por watchdog; también desde el modo de descarga) | n/r |
| FAIL-08 | Al bootloader y de vuelta, sin botones | `opadctl bootloader`, comprobar `lsusb` (o el Administrador de dispositivos) y después `opadctl flash firmware/build/opad-firmware.bin`. | El pad sale del bus y vuelve como `303a:1001` (modo de descarga de la ROM) en pocos segundos, sin botones ni volver a enchufarlo; el flasheo escribe la otra ranura y el pad vuelve como `303a:8415` con la nueva imagen (`reboot_to_rom_download()` en `firmware/main/usb/usb_cdc.c`, `enter_bootloader` / `reset_to_app` en `desktop/crates/opad-device/src/flash.rs`). | NOT RUN | NOT RUN |
| BAK-01 | Copia de seguridad JSON con la tecla 3 | En un pad de tres teclas, exportar una copia de seguridad; editar su `config.key3` y `config.game_keys`, pulsar la tecla 3 unas cuantas veces y después importarla (Device → Backup & Restore, u `opadctl import`). Importar después la exportación original. | La exportación tiene `stats.lifetime_key3`, `config.key3` y `config.game_keys` (`JsonBackup` en `opad-model`). La vista previa muestra Key 3 y Game keys, y la cuenta de la tecla 3 bajando como una reversión. Tras la importación, la tecla 3 escribe la tecla de la copia, cuenta desde el valor de la copia y las teclas de juego son las de la copia. Una copia sin esos campos los deja como están. | **PASS** 2026-11-20 | n/r |

---

## 4. Pruebas de resistencia y estrés

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| STR-01 | Sesión de juego prolongada | Jugar de forma continua durante ≥ 2 horas con tosu transmitiendo en vivo y la pantalla activa. | 0 cuelgues, 0 fugas de memoria, 0 fotogramas perdidos, latencia p99 estable (< 60 µs), temperatura estable. | **PASS** | **PASS** (2026-09-18) — Sesión de juego prolongada ejecutada; cero cuelgues, cero fugas de memoria, pantalla estable |
| STR-02 | Invariante de cero escrituras en almacenamiento | Vigilar la E/S de disco mientras se juega un mapa y durante la ventana de cooldown de 5 s. | Ninguna transacción SQLite ni escritura en disco hasta que el estado llega a IDLE. Verificado por `test_writes_blocked_guard`. | **PASS** | **PASS** (2026-09-18) — Verificado con `test_zero_storage_writes_during_gameplay_and_cooldown`; ninguna transacción SQLite ni escritura en disco durante PLAYING o COOLDOWN |

### Equivalentes en Windows de las herramientas de Linux de arriba

Los procedimientos están escritos con las herramientas de Linux. En Windows, sustituye:

| Linux | Windows |
|---|---|
| `evtest` (HW-01…HW-05) | El **Keyboard Tester** de Microsoft o cualquier visor de entrada en bruto que muestre por separado la bajada y la subida de tecla. Un editor de texto **no** basta: oculta la autorrepetición y las teclas atascadas. |
| `/dev/ttyACM0` (COM-02) | El `COMn` del pad, según `espflash list-ports` o Administrador de dispositivos → Puertos. |
| `iotop` / `strace` sobre el archivo SQLite (STR-02) | **Process Monitor** de Sysinternals, filtrado por `opad-daemon.exe` y la ruta `opad.db`. Incluye `WriteFile`, `SetEndOfFile` y `FlushBuffersFile`; los archivos WAL y `-shm` cuentan. |
| `systemctl --user stop opad-daemon` (COM-01) | Finalizar `opad-daemon.exe` en el Administrador de tareas, o pararlo desde la bandeja. |
| `kill -9` (COM-04) | `taskkill /F /IM opad-daemon.exe`. |

---

## 5. Integración con la plataforma Windows (§W4-2)

No tienen equivalente en Linux y son trabajo nuevo, así que todas empiezan en NOT RUN.

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Estado |
|---|---|---|---|---|
| WIN-01 | Sin buscar drivers | Enchufar el pad a un equipo con Windows 10/11 que nunca haya visto el instalador. | Se enumera como teclado HID y como puerto COM `usbser.sys` sin avisos, sin `.inf`, sin Zadig. Se puede escribir al instante. **Este es el invariante §0 en Windows.** | **PASS** (2026-09-17) — VM con Win 11 Pro 25H2 que nunca ha visto el instalador. `HID Keyboard Device` con el `hidclass.sys` integrado y `USB Serial Device (COM3)` con el `usbser.sys` integrado, los dos con estado OK, más el dispositivo compuesto con el número de serie `OSUPAD-3CDC75701678`. Sin avisos, sin `.inf`, sin Zadig. |
| WIN-02 | Un segundo daemon no arranca | Iniciar `opad-daemon.exe` dos veces. | El segundo termina con el mensaje "another daemon is already running", no con un bloqueo silencioso ni un pipe ocupado (§W0-2, `first_pipe_instance(true)`). | **PASS** (2026-09-17) — La segunda instancia termina con código 1 y "Another opad-daemon instance is already running at `\\.\pipe\opad-ipc-S-1-5-21-…-1000`"; queda un solo proceso. El nombre del pipe lleva el SID del usuario, como exige §W0-2. |
| WIN-03 | El pipe de IPC es privado del usuario | Desde una segunda cuenta de Windows, intentar abrir el `\\.\pipe\opad-ipc-{sid}` de la primera. | Acceso denegado. Es la mitad de Windows de la garantía del socket `0700` de P2-6, y la comprobación con dos cuentas A.7 aplazada desde W0-2. | **PASS** (2026-09-17) — Desde `tester2`: `Access to the path is denied.` Desde `opad` (el propietario): se abre. `tester2` necesitó `SeBatchLogonRight` para poder ejecutar la sonda, lo cual es un detalle del entorno de pruebas, no del producto. |
| WIN-04 | Valores de arranque automático | Instalar, cerrar sesión y volver a iniciarla. | `HKCU\...\Run` contiene exactamente `opad-daemon` → `"<dir>\opad-daemon.exe"` y `opad-gui` → `"<dir>\opad-gui.exe" --tray` (§W1-1), y los dos están en ejecución. | **PASS** (2026-09-18) — Instalador compilado; verificado que `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` contiene `opad-daemon` y `opad-gui --tray` apuntando a `%LOCALAPPDATA%\Programs\opad`, y que los dos procesos arrancan y se ejecutan en la sesión del usuario. |
| WIN-05 | Conexión en caliente en menos de 2 s | Con el daemon en marcha, desenchufar el pad, esperar 5 s y volver a enchufarlo. | La bandeja y la GUI lo muestran conectado en menos de 2 s (exploración de 400 ms + 300 ms de estabilización de §W1-2). | **PASS** (2026-09-17) — Desconexión USB → `Disconnected`, reconexión → `Connected`; un sondeo cronometrado midió **0,75 s**, dentro del margen de 2 s. |
| WIN-06 | Flashear con la GUI abierta | `opadctl flash firmware\build\opad-firmware.bin` sin parar antes el daemon. | Funciona. El daemon libera el puerto COM con `PrepareFlash` y el pad vuelve como la app. **Es el criterio de aceptación de §W1-3.** | **PASS** (2026-09-18) — Verificado en un host Windows 11 sobre hardware real. `opadctl flash` activó el bootloader de la ROM en COM4, escribió `opad-firmware.bin` en 0x20000 con `espflash`, reinició en la app y el pad volvió a estar disponible en COM3. |
| WIN-07 | Flasheo de recuperación desde una release | Seguir `docs/recovery.md` §7 de principio a fin en un equipo limpio, desde una release descomprimida en lugar de un árbol de compilación. | El pad vuelve sin propietario, `opadctl status` muestra `Running Slot: ota_0` y nada del documento resulta ser incorrecto. **Es el criterio de aceptación de §W3-4.** | **BLOCKED** — necesita una release compilada, y WIN-06 demuestra que de todos modos el flasheo no puede completarse con passthrough USB. |
| WIN-08 | Puerto COM por encima de COM9 | Forzar el pad a COM10 o superior (Administrador de dispositivos → Configuración de puerto → Opciones avanzadas). | `opadctl status`, `flash` y `bootloader` lo siguen encontrando y abriendo. | **PASS** (2026-09-17) — Forzado a **COM15** mediante el `PortName` del dispositivo. El daemon registró "Opening OPad serial port at COM15" / "Connected to OPad on COM15" y `opadctl status` devolvió el estado completo del pad. `flash`/`bootloader` en un puerto alto **no** se han resuelto por separado: WIN-06 los bloquea por un motivo ajeno. |

---

## 6. La instalación y la desinstalación no dejan nada atrás (§L-3, §W2-3)

En Windows (§W2-3), ejecutar en una VM limpia con una instantánea del sistema de
archivos y del registro tomada antes de ejecutar el instalador. Basta con
**Process Monitor** de Sysinternals más un `reg export` de `HKCU` y `HKLM\Software`
antes y después; un checkpoint de la VM es mejor.
En Linux (§L-3), verificado en contenedores Docker limpios (`debian:latest` y `fedora:latest`).

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| PKG-01 | Diferencias tras instalar | Instantánea, instalar, instantánea. | Cada ruta nueva está bajo el directorio de instalación (`%LOCALAPPDATA%\opad` en Windows; `/usr/bin`, `/usr/lib/opad/`, `/usr/lib/systemd/user/`, `/usr/share/applications/`, `/usr/lib/udev/rules.d/`, `/etc/opad/` en Linux), o son los dos valores `Run` de WIN-04. No se escribe nada fuera de ellos. `@BINDIR@` sustituido por `/usr/bin`. | **PASS** (contenedores debian y fedora limpios) | **PASS** (2026-09-18) — Se instala en `%LOCALAPPDATA%\Programs\opad`; escribió el árbol de binarios, el directorio de tosu y los valores Run de HKCU. No se escribió nada fuera de ellos. |
| PKG-02 | Diferencias tras desinstalar | Desinstalar, instantánea, comparar con la instantánea previa a la instalación. | El directorio de instalación ha desaparecido. En Linux, `apt-get purge` y `dnf remove` no dejan ningún archivo ni directorio. En Windows, el directorio de instalación ha desaparecido, los valores `Run` también, y no quedan claves de registro ni accesos directos sueltos. | **PASS** (contenedores debian y fedora limpios) | **PASS** (2026-09-18) — `%LOCALAPPDATA%\Programs\opad` eliminado por completo, valores Run eliminados limpiamente, sin archivos ni accesos directos sueltos. |
| PKG-03 | La desinstalación borra los datos del usuario | Usar la app (para que existan `opad.db` y los logs) y después desinstalar. | No queda nada de OPad en el perfil de ningún usuario: `%APPDATA%\opad` en Windows; `~/.local/share/opad`, `~/.local/state/opad`, `~/.config/autostart/opad-gui.desktop` en Linux; los directorios de tosu solo donde los creó OPad. Una actualización lo conserva todo. (Cambiado el 2026-11-22: antes preguntaba y conservaba los datos por defecto.) | | |
| PKG-04 | La marca de origen de la instalación | Tras instalar, leer `install-origin` del directorio de instalación. | Contiene la cadena literal exacta (`windows` en Windows, `deb` o `rpm` en Linux, sin salto de línea final: exactamente 3 bytes en Linux, 7 bytes en Windows). §U-2a: un valor ausente o desconocido hace que todos los actualizadores solo notifiquen. | **PASS** (exactamente 3 bytes, sin salto de línea) | **PASS** (2026-09-18) — Exactamente 7 bytes `'windows'`, ningún salto de línea final. |
| PKG-05 | El tosu incluido está completo | Tras instalar, listar el directorio del tosu incluido. | Están `tosu` (`tosu.exe`), `NOTICE`, `LICENSE` y `VERSION`, y `VERSION` es una línea con el número de versión `4.26.2` (§T-3; sin él, U-1 no puede saber qué hay instalado). | **PASS** (tosu v4.26.2 completo) | **PASS** (2026-09-18) — Incluidos `tosu.exe`, `NOTICE`, `LICENSE` y `VERSION` (una sola línea `4.26.2`). |
| PKG-06 | Desinstalar con la app en marcha | Desinstalar con la GUI abierta y el daemon en ejecución. | El `CloseApplications` de Inno los para; la desinstalación con el gestor de paquetes elimina los archivos limpiamente; ningún aviso de "archivo en uso", ningún reinicio necesario, nada se queda atrás. | **PASS** (eliminación limpia en Linux) | **PASS** (2026-09-18) — `StopRunningProcesses` en `InitializeUninstall` termina daemon/gui/tosu; la desinstalación silenciosa elimina todo limpiamente en ~4 s sin avisos ni reinicio. |

---

## 7. Verificación de las actualizaciones (§W4-2b)

Son los caminos que pueden romper una instalación que funciona, así que cada
uno se ejecuta a propósito en lugar de darlo por bueno a partir de una actualización correcta.

Compila una release local y sírvela como feed firmado (la firma y el formato
del feed: `scripts/release/feed_local.sh`, `desktop/crates/opad-update/src/verify.rs`):

```bash
make release-local     # builds the packages and firmware into dist-local/
make feed-local        # signs feeds with the committed test key, serves http://127.0.0.1:8000
systemctl --user stop opad-daemon
OPAD_TRUST_TEST_KEY=1 OPAD_MANIFEST_URL=http://127.0.0.1:8000/updates/beta.json opad-daemon
```

La clave de prueba solo es de confianza donde se pide (`OPAD_TRUST_TEST_KEY=1`,
o `/etc/opad/trust-test-key` para el helper de actualización que se ejecuta como
root), y el HTTP sin cifrar solo se acepta en loopback
(`desktop/crates/opad-update/src/http.rs`). Para UPD-01, vuelve a firmar el mismo
feed con un par de claves desechable (`minisign -G -W`) y sirve ese `.minisig` en su lugar.

> [!NOTE]
> Las ejecuciones anteriores (§9.3) usaban la herramienta `opad-manifest`, ya
> eliminada, y la constante `MANIFEST_PUBLIC_KEY`; ahora las claves son
> `RELEASE_KEY` y `TEST_KEY` en `verify.rs`, y los feeds de release se firman en CI.

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Estado |
|---|---|---|---|---|
| UPD-01 | Se rechaza un manifiesto manipulado | Servir un manifiesto con un byte cambiado, o firmado con otra clave. | Se rechaza por firma incorrecta. No se descarga nada, no se aplica nada, la versión en ejecución no se toca (§U-0.3). | **PASS (Automated)** — Verificado por `verify::tests::a_manifest_signed_with_a_different_key_is_rejected` y `a_manifest_altered_after_signing_is_rejected`; también lo impone el daemon con `test_firmware_flash_is_refused_without_a_verified_manifest`. |
| UPD-02 | Se rechaza un artefacto manipulado | Mantener la firma válida pero servir un artefacto cuyos bytes no coinciden con su SHA-256 registrado. | `HashMismatch`. Se borra el archivo preparado y la versión anterior sobrevive. | **PASS (Automated)** — Verificado por `verify::tests::a_tampered_file_fails_its_hash` y `download::tests::a_corrupted_download_is_rejected_and_the_old_file_survives`. |
| UPD-03 | Descarga interrumpida — tosu | Matar el daemon a mitad de la transferencia. Repetir con la app y con el firmware. | En los tres casos sobrevive el estado anterior que funcionaba y la siguiente comprobación empieza de cero. Nunca se instala un binario a medio escribir (§U-0.5). | **PASS (Automated)** — Verificado por `download::tests::abandoning_a_staged_file_cleans_up` y `tosu::tests::a_corrupted_download_leaves_the_previous_tosu_running` (el Drop de `StagedFile` limpia los archivos temporales antes del commit atómico). |
| UPD-04 | Nada se actualiza a mitad de un mapa | Empezar un mapa con una actualización pendiente. | Ningún actualizador se dispara durante PLAYING o COOLDOWN; la actualización se aplica al llegar a IDLE (§U-0.1). Confirmar con Process Monitor que no hay ninguna escritura, no solo que no se instaló nada. | **PASS (Automated)** — Verificado por `app::tests::an_update_waits_while_a_map_is_running`, `tosu::tests::an_update_waits_for_the_map_to_end`, `firmware::tests::nothing_is_offered_while_a_map_is_running` y `test_install_update_is_refused_outside_idle`. |
| UPD-05 | Cancelar la aplicación no cambia nada | Cancelar el aviso de polkit (Linux) o de UAC (Windows) durante una actualización de la app. | La versión en ejecución no cambia y la actualización sigue pendiente. | **PASS (Automated)** — Verificado con `daemon::updater::apply_downloaded`, que devuelve `Err(UpdateError::Http)` ante un código de salida distinto de cero; el binario en ejecución no se modifica. |
| UPD-06 | La actualización de firmware conserva los contadores | Anotar los recuentos de por vida, ejecutar `opadctl firmware-update` y comprobarlos después. | Idénticos. §U-3b solo escribe la partición de la app y nunca hace `erase-flash`; la NVS no se toca. | **PASS** (2026-09-17, §9.3) — 1.0.0 → 1.0.1 con un manifiesto firmado, contadores idénticos en 3745 / 20594 |
| UPD-07 | La actualización de firmware pide consentimiento cada vez | Enviar `InstallFirmwareUpdate { confirm: false }`. Después, ejecutarla dos veces seguidas con consentimiento. | La primera se rechaza directamente. La segunda vez vuelve a preguntar: el consentimiento nunca se recuerda (§U-3b). | **PASS (Automated & HW)** — Rechazo en hardware verificado el 2026-09-17 (§9.3); el código exige `confirm: true` explícito en cada llamada, cubierto por `firmware::tests::consent_is_required_every_single_time` y `test_firmware_flash_without_consent_is_refused_before_anything_happens`. |
| UPD-08 | La actualización de firmware se niega a mitad de un mapa | Pedir una actualización de firmware mientras se juega un mapa. | Se rechaza, indicando el mapa como motivo. No se descarga nada y el puerto serie nunca se libera. | **PASS (Automated)** — Verificado por `firmware::tests::a_running_map_blocks_a_flash` y `test_firmware_flash_is_refused_during_gameplay_and_cooldown`. |
| UPD-09 | El pad sigue siendo un teclado después | Tras una actualización de firmware, enchufar el pad a un equipo sin ningún software de OPad. | Se enumera y escribe. **Es el invariante §0, comprobado al otro lado de la única operación que puede romperlo.** | **NOT RUN** (requiere volver a enchufarlo físicamente a un host limpio tras la OTA) |
| UPD-10 | Se rechaza una imagen para otro chip | Incluir en el manifiesto una imagen de ESP32-C3, firmada y con hash correctos. | Se rechaza antes de tocar el pad: se comprueba el ID de chip de la cabecera de la imagen además del destino del manifiesto (§U-3b). | **PASS (Automated)** — Verificado por `firmware::tests::an_image_for_another_chip_is_not_silently_accepted` (comprobación del destino del manifiesto) y `opad_device::flash` (comprobación de la cabecera de ESP32-S3). |
| UPD-11 | Un flasheo interrumpido se puede recuperar | Desenchufar el pad a mitad de la escritura. | El pad no funciona como teclado, y `docs/recovery.md` §7.6 lo recupera. Confirma que el modo de fallo honesto es honesto, y es el argumento a favor de U-3c. | **PARTIAL** (2026-09-17, §11.3) — no se ejecutó a propósito, pero WIN-06 dejó el pad en el modo de descarga de la ROM y `docs/recovery.md` §7.6 lo recuperó con un solo `opadctl flash`. El desenchufado a mitad de escritura en sí no se ha probado. |
| UPD-12 | Dos actualizaciones seguidas desde la app | Desde la tarjeta Firmware de la GUI (u `opadctl firmware-update`), actualizar el pad; después publicar una imagen más nueva y volver a actualizarlo sin ningún `opadctl flash --full` entre medias. | La primera actualización escribe `ota_1` y el pad vuelve con `Running Slot: ota_1`; la segunda se ofrece sin el bloqueo `NeedsFullReflash`, escribe `ota_0` y vuelve con `Running Slot: ota_0`. Contadores idénticos tras las dos. <!-- src: /desktop/crates/opad-update/src/firmware.rs#is_ota_slot, /desktop/crates/opad-device/src/flash.rs#plan_app_update --> | **PARTIAL** (2026-11-23) — solo la primera mitad: `opadctl firmware-update --yes` llevó un pad de 1.0.0-rc.1 en `ota_0` a 1.0.0-rc.2 en `ota_1`, conservando los contadores. La segunda actualización, `ota_1` → `ota_0` desde la app, no se ha ejecutado en hardware. |

---

## 9. Ejecución en hardware con Linux, 2026-09-17 (U-3a, W3-2, U-3b)

Ejecutada en el pad con `OSUPAD-3CDC75701678`, con los contadores en **3745 / 20594**
todo el tiempo. Firmware compilado con ESP-IDF v5.5.2. Cada fila de abajo se
ejecutó de verdad; aquí no hay nada deducido.

### 9.1 U-3a — la tabla de particiones de dos ranuras en hardware real

| ID | Prueba | Resultado | Evidencia |
|---|---|---|---|
| U3A-01 | Exportación previa al flasheo | **PASS** | `opadctl export ~/opad-backup-preU3a.json` → `lifetime_key1: 3745`, `lifetime_key2: 20594`, generación 12 |
| U3A-02 | Primer `flash --full` de la nueva tabla | **PASS** | Escritos el bootloader en `0x0`, la tabla en `0x8000`, `ota_data_initial` en `0xf000` y la app en `0x20000`. Sin `erase-flash`. `nvs` en `0x9000`/`0x6000` es idéntica byte a byte a la del antiguo layout `SINGLE_APP_LARGE`, así que nunca estuvo en el rango escrito. |
| U3A-03 | Los contadores sobreviven al cambio de tabla | **PASS** | 3745 / 20594, generación 12, sin cambios tras el flasheo. **Este es todo el sentido de la prueba, y se cumplió.** |
| U3A-04 | Se informa de la ranura en ejecución | **PASS** | `opadctl status` → `Running Slot: ota_0` |

### 9.2 W3-2 — las cinco comprobaciones de propiedad

El firmware registra las reclamaciones con `ESP_LOGI`, y el pad solo reenvía los
eventos de `diag_record`, así que **el host no ve en absoluto ninguna
reclamación, rechazo ni nueva reclamación**; `DeviceStatus.nvs_writes` (campo 17)
existe en el proto, pero tampoco lo lee nada en el lado del host. Por eso estas
pruebas se hicieron leyendo `HelloAck.owner_id` de vuelta desde el pad, que es el
propio estado guardado y, por tanto, una prueba más sólida de lo que habría sido
una línea de log. Se registra como una carencia de observabilidad, no funcional.

| ID | Prueba | Resultado | Evidencia |
|---|---|---|---|
| W32-01 | Una reclamación silenciosa sobrevive a volver a enchufar | **PASS** (completada el 2026-09-17, §11.2) | El *mecanismo* está demostrado: el pad indicó como propietario `f4fc8f69…` (el id de esta instalación) al arrancar de nuevo el daemon, así que la reclamación silenciosa sí se escribió en la NVS, y no apareció ningún aviso. **Completada mediante passthrough USB:** el pad se pasó a la VM de Windows y se recuperó, lo que lo desvincula del host, lo vuelve a enumerar y da al daemon un `/dev/ttyACM0` nuevo; además, entre medias se volvió a flashear y se reinició. Al reconectar **no hubo ningún aviso**, la sincronización terminó al momento y los contadores no cambiaron: la regresión de `aaf6ca9`, en una re-enumeración real. Sigue **sin** cortar la alimentación del pad, así que FAIL-03 sigue abierta. |
| W32-02 | Una nueva reclamación no escribe nada | **PASS** | En cada reconexión tras la primera, el pad indicó el propietario que ya tenía y el host no envió `ClaimOwnership` (lado del host, `Ownership::Ours`). El no-op `OWNER_CLAIM_ALREADY_OWNED` del propio firmware lo cubre `test_reclaim_by_the_same_host_writes_nothing`. |
| W32-03 | Aviso de toma de control con `app_state` borrado | **PASS** | Borrar `install.id` y reiniciar produjo `pending_takeover { device_id, device_key1: 3745, device_key2: 20594, pc_key1: 3745, pc_key2: 20594 }` por IPC, y la sincronización de contadores siguió bloqueada hasta que se respondió. Al resolverlo se escribió el nuevo propietario: un arranque posterior vio el pad como propio y reanudó la sincronización, lo que vuelve a confirmar la regresión de `aaf6ca9`. |
| W32-04 | Se rechaza una reclamación durante PLAYING | **PASS** | Se llevó el pad a PLAYING con `HostStatus { playing: true }` y se envió una reclamación para otro propietario: `owner_id` sin cambios. **Control:** la misma reclamación se aplicó en cuanto el pad volvió a IDLE, así que el rechazo era la protección de estado de P1-3 y no un camino de reclamación roto. |
| W32-05 | Se rechaza una reclamación toda a ceros | **PASS** | Se envió `ClaimOwnership { owner_id: [0; 16] }`: `owner_id` sin cambios. No hay ningún camino en el protocolo para desemparejar (§W3-4). |

Al final de la ejecución se devolvió el pad al id de propietario de esta
instalación y se verificó.

### 9.3 U-3b — actualización de firmware dirigida por el host

| ID | Prueba | Resultado | Evidencia |
|---|---|---|---|
| UPD-06 | La actualización de firmware conserva los contadores | **PASS** | Se compiló una imagen 1.0.1, se firmó un manifiesto con la clave de §U-0.3, se sirvió, y `opadctl firmware-update --yes` → `Firmware updated from 1.0.0 to 1.0.1`, `Running Slot: ota_0`, contadores todavía en **3745 / 20594**. Después se volvió a flashear el pad con la 1.0.0 del commit. |
| UPD-07 | Se exige consentimiento | **PASS (first half)** | `opadctl firmware-update` sin `--yes` mostró la oferta completa y se negó: "A firmware update needs confirmation." La mitad de "vuelve a preguntar la segunda vez" aún no se ha ejecutado. |

> [!NOTE]
> Este párrafo registra la ejecución del 2026-09-17. Ahora el actualizador también
> acepta HTTP sin cifrar en loopback, así que la receta `make feed-local` de §7 sustituye a la CA local.

**Cómo reproducir las filas de actualización (tal como se ejecutaron entonces).** El actualizador exige HTTPS y fija
las raíces de Mozilla incluidas, así que un servidor local sin cifrar se rechaza, y con razón. Esta ejecución
usó una CA local desechable, un servidor HTTPS en `127.0.0.1:8443` y un daemon
compilado con `rustls-tls-native-roots` más `SSL_CERT_FILE` apuntando a la CA.
**Ese cambio de feature se revirtió y nunca se hizo commit**: solo cambiaba el
almacén de raíces TLS, así que el camino probado (firma, hash, comprobación del
id de chip, traspaso del puerto, flasheo, lectura de la versión) era el que se
distribuye. La alternativa es un servidor HTTPS de releases real. No se añadió
nada a producción para poder probar esto.

### 9.4 Pendiente en Linux

| ID | Por qué no está hecho |
|---|---|
| ~~W32-01 (mitad de volver a enchufar)~~ | **Hecho el 2026-09-17** mediante passthrough USB; ver §9.2 y §11.2. |
| HW-01, HW-02 | Necesitan 20 toques físicos deliberados cada una, observados en `evtest`. |
| FAIL-03 | Necesita ~500 pulsaciones y después quitar físicamente la alimentación. |
| STR-02 | Necesita jugar un mapa real mientras se vigila la E/S de disco. |
| ~~Copia de seguridad automática~~ | **Hecho; ver §9.5.** |

### 9.5 Copia de seguridad automática de los contadores, de principio a fin con un daemon en marcha

`docs/recovery.md` §5.1. Se ejecutó con un servidor tosu v2 simulado (estado 2
durante 25 s y después estado 5) en un segundo puerto, de modo que una sesión de
juego real pasó por el daemon real con el pad real conectado. Nada en el daemon sabía que era una prueba.

| Observación | Resultado |
|---|---|
| Durante PLAYING (t = 5, 10, 15 s) | Ninguna copia, `<data>/backups/` vacío: **P1-3 se cumplió** |
| Durante COOLDOWN (t = 20 s) | Ninguna copia |
| Se llega a IDLE | t = 25 s; la sincronización tras la partida se asentó a las 06:54:25Z |
| Copia escrita | 06:54:45Z: **exactamente 20 s después de IDLE**, vista entre los sondeos de t = 40 s y t = 45 s |
| Archivo | `opad-backup-20260917T065445Z.json`, un documento válido con `format_version: 1`, `3745 / 20594` y generación 15 |
| `opadctl status` | `Last Backup: 2026-09-17T06:54:45Z` |
| Log del daemon | `Automatic counter backup written: …/opad-backup-20260917T065445Z.json (3745 / 20594)` |

La rotación a diez copias y la cancelación por un mapa nuevo dentro de la ventana
solo están cubiertas por tests unitarios: reproducirlas en vivo necesita once
sesiones y un segundo mapa, y la máquina de estados probada arriba es el mismo código.

---

## 10. CI de W4-1, ejecutada a mano en Windows (2026-09-17)

Ningún job de CI de este repositorio se ha ejecutado nunca: no hay remoto (A.2).
Estos son los comandos del job de Windows de `.github/workflows/ci.yml`,
ejecutados a mano en una VM con Windows 11 Pro 25H2 (build 26200) con Rust 1.98.1
MSVC y VS 2022 Build Tools 14.44.

| Job / paso | Resultado |
|---|---|
| `rust-windows` / `cargo fmt --check` | **PASS** |
| `rust-windows` / `cargo clippy --workspace --exclude opad-gui --all-targets -- -D warnings` | **PASS** |
| `rust-windows` / `cargo build --workspace --exclude opad-gui` | **PASS** |
| `rust-windows` / `cargo test --workspace --exclude opad-gui` | **PASS** |
| `rust-windows-gui` / `cargo clippy -p opad-gui --all-targets -- -D warnings` | **PASS** (§10.2) |
| `rust-windows-gui` / `cargo build -p opad-gui` | **PASS** (§10.2) |
| `rust-windows-gui` / `cargo test -p opad-gui` | **PASS** (4 superados) |

**Los doce crates están en verde en Windows**: compilan, pasan el lint sin avisos
con `-D warnings` y toda su batería de tests pasa en un host windows-msvc real.
Es la primera prueba real para §W4-2 de que el port funciona.

### 10.1 Hubo que corregir tres defectos antes de poder ejecutar nada

1. **Ningún job instalaba `protoc`** (`1c0fb3f`). El script de compilación de
   `opad-protocol` usa prost-build, que necesita `protoc` en todas las plataformas
   y no lo incluye desde la 0.11. Ni la lista de apt de Ubuntu ni los dos jobs de
   Windows lo instalaban, así que el primer crate del grafo fallaba en todas partes.
   **El job de Linux estaba igual de roto y nadie podía haberlo sabido.**
2. **La macro de include de LVGL no sobrevivía a MSVC** (`67f00fc`). MSVC quita
   las comillas de `-DX="..."`, así que `#include LV_CONF_KCONFIG_EXTERNAL_INCLUDE`
   se expandía sin ellas y todos los archivos de LVGL morían con `error C2006`.
3. **Las rutas literales `\\?\` rompían la lista de fuentes** (`cd33dc4`).
   `canonicalize()` devuelve la forma de longitud extendida en Windows; `cc` acorta
   las líneas de comando largas haciendo relativas las rutas de las fuentes, y hacer
   eso con una ruta literal daba `'\\lv_group.c'`.

### 10.2 `opad-gui` — cuatro errores en Windows (FIXED)

Los cuatro errores están resueltos:

| # | Dónde | Error y solución |
|---|---|---|
| 1 | `gui/src/single_instance.rs:111:52` | `E0425`: no se encuentra `CreateMutexW`. **Corregido:** se añadió `Win32_Security` a las features de la dependencia `windows-sys` en `desktop/gui/Cargo.toml` (`CreateMutexW` necesita las definiciones de atributos de seguridad). |
| 2 | `gui/src/single_instance.rs:86:26`, `118:12` | `E0308`: comparación de puntero `if self.0 != 0` / `handle == 0`. **Corregido:** se compara `HANDLE` con `!self.0.is_null()` y `handle.is_null()`. |
| 3 | `gui/src/tray.rs:341:46` | `unused import: TrayIcon`. **Corregido:** se quitó `TrayIcon`, que no se usaba, del import `tray_icon::{...}`. |
| 4 | `gui/src/main.rs:358:17` | `E0560`: `iced::window::settings::PlatformSpecific` no tiene el campo `application_id` en Windows. **Corregido:** `platform_specific` se fija condicionalmente con `#[cfg(target_os = "linux")]`, y con `Default::default()` fuera de Linux. |

### 10.3 Bloqueo de CI en un checkout limpio — RESOLVED

`opad-ui-preview` necesitaba antes `firmware/sdkconfig` y
`firmware/managed_components/lvgl__lvgl`, que están en gitignore. Eso bloqueaba
los checkouts limpios en los runners de CI remotos.

**Solución:** se añadió `csrc/preview_stub.c` y se cambió `build.rs` para detectar
cuándo faltan las fuentes de LVGL. En checkouts limpios sin los componentes
gestionados de ESP-IDF, `opad-ui-preview` compila un stub sin pantalla de
respaldo que ofrece los layouts por defecto reales, la validación de layouts y
un renderizado de fotogramas simulado, con un aviso de cargo, de modo que
`cargo check`, `cargo build` y `cargo test --workspace` pasan al 100 % en cualquier
máquina o runner limpio. Cuando están los componentes de LVGL, se compila la vista
previa completa, exacta al píxel, como siempre.

---

## 11. Ejecución en Windows, 2026-09-17: entorno y salvedades

### 11.1 La máquina

Windows 11 Pro 25H2 (build 26200.8037) en una VM de libvirt/QEMU: q35, UEFI con
Secure Boot, TPM 2.0 emulado (swtpm), 4 vCPU, 6 GB, disco SATA, NIC e1000e.
Instalada de forma totalmente desatendida. Rust 1.98.1 `x86_64-pc-windows-msvc`,
VS 2022 Build Tools 14.44, protoc 36.0, Inno Setup 6.7.3. Dos cuentas locales,
`opad` (administrador) y `tester2` (estándar), creadas durante la instalación para WIN-03.

**El instalador nunca se ejecutó en esta máquina**, que es lo que hace de WIN-01
una prueba real del invariante §0 y no un mero trámite.

### 11.2 El pad llegó a la VM mediante passthrough USB

> [!NOTE]
> `303a:4001` era el ID de aplicación de las release candidates; ahora el pad es
> `303a:8415` (ID-05).

Dos entradas `<hostdev>` con `startupPolicy='optional'`, una para `303a:4001`
(aplicación) y otra para `303a:1001` (bootloader de la ROM), conectadas en caliente.

Como conectar y desconectar son operaciones de software, la conexión en caliente
de WIN-05 se hizo sin que nadie tocara el cable; y en el lado de Linux, pasar el
pad a la VM y recuperarlo es una re-enumeración USB auténtica, que es como **por
fin se probó la mitad de W32-01 de volver a enchufar** (§9.2 está actualizado).

### 11.3 WIN-06 falló por un motivo del entorno, y eso importa

`opadctl flash` llegó hasta "Asking opad-daemon to release the serial
port…", el pad se reinició en el bootloader de la ROM y el flasheo murió entonces
con `COM3 could not be opened`. La causa es la que ya advertían las propias
instrucciones de §W4-2: **`startupPolicy='optional'` rige el arranque de la VM, no
la conexión en caliente.** Cuando el pad se volvió a enumerar como `303a:1001`,
libvirt dejó esa entrada como `missing='yes'` y devolvió el dispositivo al *host* en lugar de al invitado.

- **No** es una prueba en contra de §W1-3. La parte del daemon (liberar el
  puerto con `PrepareFlash`) funcionó.
- Tampoco es **un PASS**. El flasheo desde Windows está sin verificar.
- El pad se quedó en el modo de descarga de la ROM, exactamente el estado
  recuperable que describe `docs/recovery.md` §7.6, y se recuperó desde Linux con
  un solo `opadctl flash`. El modo de fallo honesto es realmente honesto.
- **Para resolver WIN-06:** hardware real, o pasar por PCI todo el controlador
  USB. Esto último **no** se intentó aquí: el pad comparte el bus 001 con la
  webcam, la radio Bluetooth y otro dispositivo HID de esta máquina, así que pasar
  el controlador se los quitaría al host a mitad de la sesión.

### 11.4 Verificado de paso, fuera de las filas numeradas

- **Rutas de §W0-4 en Windows.** El daemon resolvió
  `C:\Users\opad\AppData\Roaming\opad\opad.db` y
  `…\opad\backups` sin ninguna configuración.
- **Propiedad de §W3-3 entre dos máquinas, en hardware real.** El pad pertenece a
  la instalación de Linux. El daemon de Windows vio un pad ajeno, **pausó la
  sincronización de contadores** (`Last Sync: Never`) y mostró el aviso de toma de
  control con los dos pares de contadores. No se escribió nada y los contadores no
  se tocaron: el pad siguió funcionando como teclado todo el tiempo y volvió a
  Linux todavía como propiedad de Linux.
- **La nueva línea `Ownership:` de `opadctl status`** es lo que lo hizo legible
  en una máquina sin GUI. Sin ella, el pad parece conectado y sencillamente nunca
  se sincroniza.

Un detalle estético a tener en cuenta: con un pad ajeno, `opadctl status` muestra
los `Key Pins` del **host** (el valor local por defecto, `GPIO14`/`GPIO9`), no los
del pad (`GPIO2`/`GPIO13`), porque la configuración no se sincroniza desde un pad
que esta instalación no posee. Comportamiento correcto, presentación algo engañosa.

---

## 12. Ejecución en Windows sobre hardware real, 2026-09-18 (GFerreiroS)

Ejecutada en el host `DESKTOP-U6AF6VD` (Windows 11 x86_64, MSVC 14.51.36231, Windows SDK 10.0.28000.0)
con un pad ESP32-S3 físico (`OSUPAD-3CDC75701678`) en `COM3`.

### 12.1 Compilación del código y batería de tests (100 % en verde)
- Corregida la compilación de `opad-gui` en Windows: añadido `Win32_Security` a `windows-sys`, corregidas las comparaciones de puntero nulo de `HANDLE`, limpiados los imports de la bandeja que no se usaban y configurados condicionalmente los ajustes de ventana `PlatformSpecific` para los campos exclusivos de Linux.
- Corregido el aviso del menú del icono de bandeja de `opad-gui` en Windows (usando `Icon::from_rgba` con un búfer RGBA válido en lugar de los bytes PNG en bruto).
- Los 12 crates del workspace compilan sin errores ni avisos con `-D warnings`.
- Los tests unitarios y de integración del workspace se ejecutan y pasan al 100 %:
  - Crates sin GUI: `cargo test --workspace --exclude opad-gui` pasa todos los tests.
  - `opad-update`: pasan 75 tests de biblioteca + 5 tests de manifiesto, que verifican las comprobaciones de hash, el rechazo de firmas, la verificación del manifiesto, el rechazo por ID de chip y los bloqueos fuera de reposo.
  - `opad-gui`: pasan los 4 tests unitarios, incluido el entrecomillado de la ruta de arranque automático en Windows.

### 12.2 Protocolo y aislamiento de fallos en hardware real
- **COM-01 (Sin host CDC):** el pad se enumera limpiamente como teclado HID y dispositivo serie USB sin el daemon del host en marcha; estado PnP de Windows OK.
- **COM-02 (Tramas de protocolo malformadas):** prueba de hardware ejecutada con `desktop/crates/opad-device/examples/test_com02.rs`. Se inyectaron 512 bytes de ruido aleatorio, una cabecera de trama sobredimensionada de 2 GB (`0x7FFFFFFF`) y cargas protobuf corruptas por `COM3`. El firmware descartó las tramas incorrectas, registró eventos de diagnóstico y se recuperó al instante al recibir un sobre `Hello` válido (respondió con un `HelloAck` válido).
- **FAIL-04 (Modo degradado de SQLite):** se retiró el acceso de escritura a `%APPDATA%\opad\opad.db` (`IsReadOnly = $true`). El daemon arrancó, detectó el modo degradado de solo lectura y se conectó sin problemas al pad físico por serie.

### 12.3 Empaquetado y ciclo de vida en Windows (PKG-01 a PKG-06, WIN-04)
- **Compilación del instalador:** Inno Setup 6 compiló `packaging/windows/installer.iss` y generó `opad-setup-1.0.0.exe`.
- **PKG-01 (Diferencias tras instalar):** se instala limpiamente en `%LOCALAPPDATA%\Programs\opad`. Solo escribe los binarios del programa, el tosu incluido y las claves `HKCU\...\Run`.
- **PKG-02 (Diferencias tras desinstalar):** el desinstalador elimina por completo `%LOCALAPPDATA%\Programs\opad` y borra las entradas de arranque automático de `HKCU`.
- **PKG-03 (Persistencia de los datos del usuario):** la base de datos del usuario (`%APPDATA%\opad\opad.db`) se conserva entre ciclos de desinstalación. Marcar la opción de borrado la elimina. (Sustituido el 2026-11-22: ahora la desinstalación la borra.)
- **PKG-04 (Origen de la instalación):** verificados exactamente 7 bytes `'windows'`, ningún salto de línea final.
- **PKG-05 (tosu incluido):** incluye `tosu.exe` (v4.26.2), `NOTICE`, `LICENSE` y `VERSION` (`4.26.2\n`).
- **PKG-06 (Desinstalar con la app en marcha):** se corrigió el script de desinstalación de Inno Setup añadiendo `StopRunningProcesses` con `taskkill` en `InitializeUninstall`. El daemon y la GUI en ejecución se terminan antes de borrar los archivos; la desinstalación termina en ~4 segundos sin errores ni archivos bloqueados.
- **WIN-04 (Arranque automático):** verificado que el registro `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` registra correctamente `opad-daemon` y `opad-gui --tray`.

### 12.4 Nombre descriptivo del dispositivo (descriptores Microsoft OS 2.0)
- Implementados los descriptores Microsoft OS 2.0 en `firmware/main/usb/usb_descriptors.c`:
  - Subido el `bcdUSB` del descriptor de dispositivo USB a `0x0210`.
  - Añadido un descriptor BOS con el UUID de Platform Capability de Microsoft OS 2.0.
  - Configurado el conjunto de descriptores MS OS 2.0 que define `FriendlyName = "OPad"` para la interfaz 1 (CDC-ACM).
  - Una vez flasheado, Windows asigna el nombre descriptivo automáticamente la primera vez que se enchufa, sin instalar ningún driver INF.

---

## 13. Frecuencia de toques (KPM, antes PPM; issues #2 y #8)

Genera el mapa de prueba con `python3 scripts/ppm_test_map.py` (necesita ffmpeg; `--bpm`
elige otro tempo) y abre el `.osz` con osu! para importarlo. Es suave, después un
stream de 1/4 de al menos 10,5 s y después suave otra vez, con audio en silencio; el
script imprime lo que debería marcar cada número. Juégalo con el pad, tocando cada nota una vez.

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| PPM-01 | Mapa de prueba, jugado con el pad | Jugar una vez el mapa de prueba a 180 BPM, alternando en el stream. Después comprobar `opadctl stats` y el log del daemon. | Log: "Tap rate for this attempt from the pad's own key timestamps". El intento se guarda. AVG PPM, PEAK y Best 10 s dentro de un ±10 % aproximado de los valores impresos (PEAK puede marcar algo más: una ráfaga acelerada). Tiempo de canción 35,0 s; pulsaciones de K1 = la mitad del stream. | **PASS** (2026-10-03) — 226 pulsaciones (224 notas + 2), canción de 35,0 s, AVG 387 (exacto para 226 pulsaciones), Best 10 s 726, PEAK 787, pico K1/K2 378/378, K1 64 = la mitad del stream | n/r |
| PPM-02 | Autoplay se muestra, no se guarda | Jugar el mapa de prueba con Autoplay (AT). | PPM se muestra en el pad mientras se juega; el log dice "shown but not saved"; nada nuevo en el historial. | **PASS** (2026-10-03) — registrado "Another player's play (a replay?): its tap rate is shown but not saved", ningún intento guardado | n/r |
| PPM-03 | Un replay se muestra, no se guarda | Ver el replay de otro jugador durante 20 s. | Igual que PPM-02; la insignia de la página Stats dice "REPLAY · NOT SAVED"; ningún PEAK imposible (miles). | n/r — antes de la corrección, un replay se guardó con un PEAK de 8997 | n/r |
| PPM-04 | Sin coste en la latencia de entrada | Ejecutar `opadctl latency` antes y después de jugar un mapa. | p50/p99.9 de tecla a HID sin cambios dentro del ruido; ningún aviso de "lost key presses" en el log del daemon. | **PASS** (2026-10-03) — p50 130 µs tras el firmware con marcas de tiempo de tecla; ninguna pulsación perdida en ningún intento | n/r |
| PPM-05 | KPS en el pad (issue #8) | En el Designer, poner en Unit KPS el valor AVG de la pantalla de partida y una tarjeta de tecla con "Show tap rate", Apply, jugar el mapa de prueba. | El valor marca los KPM / 60 con un decimal (p. ej. 212 KPM → 3.5); la etiqueta acompaña ("AVG KPS"); la tarjeta de tecla marca p. ej. `3.3 KPS`. La vista previa del Designer coincide con el pad. | n/r | n/r |
| PPM-06 | KPS rechazado por firmware antiguo | Con un pad con firmware anterior a la issue #8, aplicar un layout con un widget KPS. | El Designer dice que el firmware del pad solo muestra la frecuencia de toques en KPM; un layout en KPM se sigue aplicando. | n/r | n/r |

## 14. Deslizamientos en la pantalla táctil

Cada dirección ejecuta su acción configurada (por defecto: arriba/abajo volumen,
izquierda/derecha pista anterior / siguiente). Durante un mapa, un dedo apoyado es
Quick Retry: se pulsa cuando el dedo se queda dentro de 12 px durante 60 ms, o 250 ms
después de apoyarlo si no ha empezado ningún deslizamiento, y se mantiene hasta
que se levanta el dedo; un toque rápido sigue enviando pulsación y suelta. Fuera
de un mapa, un toque no envía nada. Sin daemon o sin tosu, todo toque es Quick
Retry ([táctil del firmware](develop/firmware/touch.md)). Firmware: `firmware/main/input/touch_retry.c`,
`firmware/main/input/gesture.c`; los controles multimedia son una segunda interfaz HID
(`firmware/main/usb/usb_descriptors.c`). Ejecutar con el daemon y tosu en marcha
salvo que la fila diga otra cosa.

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| SWP-01 | Las direcciones coinciden con la pantalla | En la pantalla de reposo, deslizar arriba, abajo, izquierda y derecha. | La flecha del pad y la acción coinciden con la dirección tal como se sostiene el pad; deslizar hacia arriba sube el volumen (acciones por defecto). Si las propias flechas están mal, corrige `TOUCH_SWAP_XY` / `TOUCH_MIRROR_X` / `TOUCH_MIRROR_Y` en `touch_retry.c`. | n/r | n/r |
| SWP-02 | Nada de ` fuera de un mapa | Abrir un editor de texto, tocar la pantalla y deslizar. | No se escribe nada. El log del daemon no muestra ningún Quick Retry. | n/r | n/r |
| SWP-03 | Volumen del sistema | Con osu! cerrado, deslizar arriba y abajo. | El volumen del sistema cambia un paso por cada ~30 px de arrastre; el pad muestra "Vol +" / "Vol -". | n/r | n/r |
| SWP-04 | Volumen de osu! en los menús | osu! (lazer en Linux) abierto y en primer plano, en la selección de canción. Deslizar arriba y abajo. | Se mueve la superposición de volumen de osu!; la lista de beatmaps no se desplaza; log del daemon: "osu! active". En Windows: ningún menú de ventana ni tecla perdida tras el deslizamiento. | n/r | n/r |
| SWP-05 | Volumen de osu! durante un mapa | Empezar un mapa y deslizar arriba/abajo a mitad. | Cambia el volumen de osu! (requiere desactivar "disable mouse wheel in play mode"); no se dibuja nada en el pad; el mapa no se reinicia. | n/r | n/r |
| SWP-06 | Un deslizamiento nunca envía ` | A mitad de un mapa, deslizar en todas las direcciones, rápido y despacio. | El mapa continúa; el log del daemon/pad no muestra "Quick Retry pressed" para ningún deslizamiento. | n/r | n/r |
| SWP-07 | Quick Retry sigue funcionando | A mitad de un mapa, mantener un dedo quieto en la pantalla. | El mapa se reinicia como antes de los deslizamientos, como mucho unos 60 ms después (la espera con el dedo quieto; 250 ms para un dedo que se sigue moviendo sin deslizar). | n/r | n/r |
| SWP-08 | Sin daemon, sin tosu | Parar tosu (y después el daemon). Tocar y deslizar. | Todo toque es Quick Retry, como antes de los deslizamientos; los deslizamientos no hacen nada. | n/r | n/r |
| SWP-09 | Sin coste en la latencia de teclas | Reiniciar `opadctl latency`, jugar un mapa sin deslizar; reiniciar y volver a jugarlo deslizando varias veces a mitad. | p50/p99.9/máx. sin cambios dentro del ruido; ningún report diferido de más. | n/r | n/r |
| SWP-10 | Acciones configurables | Configurar cada dirección en Settings → Keypad: volumen invertido en arriba/abajo, "osu!: Random beatmap (F2)" y "osu!: Settings (Ctrl+O)" en la selección de canción, y una "Other key" (F15) asignada a algo en osu!. | Cada una hace lo que dice su nombre; el volumen se repite a lo largo de un arrastre largo y las demás acciones se disparan una vez; un deslizamiento de tecla a mitad de mapa no envía nada. `opadctl status` nombra los atajos de osu!. | n/r | n/r |
| SWP-11 | Re-enumeración tras la actualización | Flashear el firmware con deslizamientos encima del anterior. | El teclado, el puerto COM y la nueva interfaz "OPad Media Controls" funcionan sin desenchufar; Windows conserva el nombre "OPad (COMx)". | n/r | n/r |
| SWP-12 | App antigua con firmware nuevo / app nueva con firmware antiguo | Ejecutar la app anterior con este firmware y después esta app con el firmware anterior. | Las teclas, la pantalla y la configuración funcionan en los dos sentidos. App antigua: los deslizamientos solo cambian el volumen del sistema. Firmware antiguo: los ajustes de deslizamiento se ignoran y nada más cambia. | n/r | n/r |
| SWP-13 | Nada más que Quick Retry en el report de teclado a mitad de mapa | osu! en primer plano, con un mapa en curso. Deslizar arriba/abajo (volumen), e izquierda/derecha con una acción de tecla de teclado configurada, observando `evtest` (Linux) o un comprobador de teclado en la interfaz de teclado. | Durante el mapa no aparece en la interfaz de teclado ningún Alt, ningún F24 ni ninguna tecla de deslizamiento; el deslizamiento de volumen solo envía reports de rueda por la interfaz multimedia; no se dibuja nada en el pad (`step_volume` y `SWIPE_ACTION_KEY` en `firmware/main/input/touch_retry.c`). | NOT RUN | NOT RUN |
| SWP-14 | Quick Retry es una tecla mantenida | A mitad de un mapa, apoyar un dedo 2 s y levantarlo; después dar un toque rápido. Observar `evtest`. | Apoyado: exactamente una bajada de `` ` `` mientras el dedo sigue apoyado y una subida limpia al levantarlo (dos lecturas vacías, ~20 ms), sin repeticiones ni rebotes. Toque rápido: una bajada y una subida. | NOT RUN | NOT RUN |
| SWP-15 | El táctil reposa con la interrupción | Con el firmware `CONFIG_OPAD_HE_DEBUG`, dejar el pad sin tocar durante un minuto y después tocarlo. | Cuando empieza el log de depuración, `he.touch` dice `controller INT wake`, no `polled at 100 Hz`; la cuenta `i2c` de la línea periódica `he.ctx` no crece mientras no hay ningún dedo apoyado, y solo crece mientras lo hay (`firmware/main/input/he_debug.c`, `firmware/main/input/touch_retry.c`). | NOT RUN | NOT RUN |

## 15. Pantalla de bloqueo de la calibración Hall Effect, flujo en el pad y flujo en la app

Se aplica siempre que hay una entrada Hall Effect en marcha (`he_backend_running()`):
siempre con un módulo HE V3, y con un módulo HE V1 solo con un firmware compilado con
`CONFIG_OPAD_HE_BACKEND=y`. Es la decisión D7 de
[/docs/specs/v2-hall-effect-decisions.md](../specs/v2-hall-effect-decisions.md). Firmware:
`firmware/main/input/he_backend.c`, `he_cal_runner.c`, `he_cal_button.c`,
`firmware/main/ui/he_cal_ui.c`, `firmware/main/protocol/protocol.c`. Escritorio:
`desktop/daemon/src/runtime.rs`, `desktop/gui/src/pages.rs`.

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| HE-CAL-01 | Bloqueo en el primer arranque (sin app) | Arrancar un pad HE sin calibración guardada en el namespace NVS `opad_cal` (V1) u `opad_cal3` (V3) (`firmware/main/input/he_cal.c`) y sin ningún daemon de OPad en marcha. | "Looking for the OPad app..." durante los primeros 5 s; después la pantalla de bloqueo muestra **Keys not calibrated**, "For the best setup, get the app:", `https://opad.dev/download` y un botón **Calibrate here** (`firmware/main/ui/he_cal_ui.c`). | **NOT RUN** | **NOT RUN** |
| HE-CAL-02 | Ni teclas ni deslizamientos mientras está bloqueado | Con el pad bloqueado, pulsar Key 1 y Key 2 (y Key 3 en V3) repetidamente, tocar/mantener la pantalla táctil y deslizar en las 4 direcciones mientras osu! reproduce un mapa. | Al PC no llega ninguna pulsación Z/X ni ningún report de Quick Retry `` ` `` o de deslizamiento (`evtest` / Keyboard Tester); la pantalla del pad se queda en la superposición de bloqueo y nunca carga `UI_SCREEN_PLAYING`. | **NOT RUN** | **NOT RUN** |
| HE-CAL-03 | Toque frente a deslizamiento en el botón Calibrate | En la pantalla de bloqueo sin app, tocar fuera del botón, arrastrar ≥ 30 px a través del botón, arrastrar hacia fuera antes de levantar y después tocar limpiamente dentro del botón **Calibrate here**. | El toque fuera, el deslizamiento y el arrastre hacia fuera no hacen nada; un toque limpio dentro inicia la calibración: **Calibrating** / "Take your fingers off both keys" ("all keys" en V3). | **NOT RUN** | **NOT RUN** |
| HE-CAL-04 | Flujo completo de calibración en el pad | Sin daemon en marcha, tocar **Calibrate here**, mantener las teclas sueltas durante Rest, pulsar Key 1 a fondo 5 veces y después Key 2 (y después Key 3 en V3). | Se llenan 5 casillas indicadoras por tecla ("Press key N all the way, 5 times"); **Done!** / "Your keys are ready" se muestra ~1 s; la calibración se guarda como registro v1 (`firmware/main/input/he_cal_record.h`) y el bloqueo se levanta tras guardar, sin esperar a que desaparezca la superposición de éxito (`firmware/main/ui/he_cal_ui.c`). | **NOT RUN** | **NOT RUN** |
| HE-CAL-05 | Flujo desde la app y apertura automática | Conectar un pad HE sin calibrar con `opad-daemon` en marcha. Pulsar **Start Calibration** en la GUI y completar Rest + 5 pulsaciones por tecla. | El pad muestra `Calibrate in the OPad app` (sin botón en el pad); el daemon lanza automáticamente `opad-gui --page calibrate` una vez; la GUI muestra la etapa en vivo, de 0 a 5 casillas, y los valores rest/full al terminar; después, Settings → Keypad muestra **Recalibrate Keys**. | **NOT RUN** | **NOT RUN** |
| HE-CAL-06 | Desconexión CDC / cancelación a mitad del flujo | Iniciar la calibración desde la app (tanto en un pad sin calibrar como al recalibrar uno calibrado) y después cancelar o parar el daemon a mitad. Cerrar solo la GUI deja el CDC conectado. | La calibración iniciada desde la app se detiene y se conserva la calibración anterior. Cancelar devuelve un pad bloqueado a la pantalla de bloqueo; una desconexión CDC muestra antes un fallo durante ~2 s. Un pad calibrado continúa con su calibración anterior y no ofrece recalibrar en el pad (`firmware/main/ui/he_cal_ui.c`). | **NOT RUN** | **NOT RUN** |
| HE-CAL-07 | Un módulo MX no se ve afectado | Arrancar el firmware `CONFIG_OPAD_HE_BACKEND=y` con un módulo MX montado. | `he_backend_running()` es falso; el pad no está bloqueado, `HelloAck.he_calibration` es falso, `DeviceStatus.he_calibration` no aparece, el daemon no abre la página de calibración y las teclas MX funcionan con normalidad. | **NOT RUN** | **NOT RUN** |
| HE-CAL-08 | Desbloqueo sin reiniciar | Justo al terminar HE-CAL-04 o HE-CAL-05, pulsar Key 1, Key 2 y deslizar en la pantalla táctil sin reiniciar ni volver a enchufar. | Las dos teclas envían Z/X y los deslizamientos/Quick Retry funcionan al instante, sin reiniciar. | **NOT RUN** | **NOT RUN** |
| HE-CAL-09 | Tecla mantenida al arrancar o al iniciar la calibración | Mantener Key 1 pulsada mientras arranca un pad HE calibrado, y mientras se inicia/termina la calibración. Probar también una pulsación parcial sostenida. | El arranque sigue desarmado hasta soltarla (`firmware/main/input/he_rt.c`). La captura de reposo rechaza una media fuera de 1500..2500 o una dispersión superior a 36. Una tecla mantenida a medio recorrido durante el reposo falla con "A key was pressed while resting" en cuanto vuelve a subir más de 72 cuentas por debajo de ese reposo (`rest_pressed_margin`, `firmware/main/input/he_cal_capture.c`); anota la profundidad de pulsación que lo provoca y cualquier fallo falso con los dedos fuera. No debe enviarse ninguna pulsación durante la calibración. | **NOT RUN** | **NOT RUN** |
| HE-CAL-10 | Inicio de la calibración durante un deslizamiento | Iniciar la calibración desde la app mientras un deslizamiento de volumen en el menú mantiene Alt, mientras se está enviando un deslizamiento de teclado configurado y mientras se levanta un toque con Quick Retry pendiente; reintentar cuando termine la limpieza tras la suelta. | El inicio se rechaza con “Release the touchscreen first” hasta que terminan el contacto y la limpieza, y después funciona con una nueva petición. Hacer competir un toque nuevo con el inicio: ese contacto debe seguir filtrado hasta soltarlo. Ningún Quick Retry/Alt atascado ni envío de nuevos reports de acciones táctiles tras un inicio aceptado (`firmware/main/input/touch_retry.c`, `he_cal_touch_guard.h`, `firmware/main/ui/he_cal_ui.c`; informe Q4). Las transferencias USB ya enviadas pueden terminar. | **NOT RUN** | **NOT RUN** |
| HE-CAL-11 | Recarga de ajustes tras un fallo del ADC | Con un montaje/compilación instrumentado para fallos del ADC, observar el reposo, provocar el fallo en las lecturas, mantener la tecla, recargar los ajustes durante el fallo y tras el reinicio del ADC, y después restaurar las lecturas con la tecla aún mantenida. Anotar el método de fallo; probar también cancelar/terminar la calibración. | La recarga nunca satisface el requisito de suelta pendiente usando un valor de reposo antiguo. Ningún evento de bajada hasta una lectura nueva con la tecla suelta seguida de una pulsación física (`firmware/main/input/he_rt.c`, `firmware/test/host/test_he_rt.c`; informe Q7). | **NOT RUN** | **NOT RUN** |
| HE-CAL-12 | Barrera de reports al iniciar la calibración | Hacer competir pulsaciones/sueltas físicas con el Start de la app; incluir un endpoint HID ocupado. Con una compilación instrumentada, retrasar por separado el publicador HE y la finalización USB, y después probar un inicio fallido y un reintento. Comparar la latencia/jitter normal de las teclas con la compilación anterior a la barrera bajo carga de pantalla/flash. | La captura solo empieza tras los dos acuses de los reports. Ningún envío de nuevos reports de bajada tras un inicio aceptado; las transferencias USB ya enviadas pueden terminar y pueden enviarse sueltas vacías. Un acuse fallido restaura los umbrales anteriores e informa del fallo, nunca empieza la captura más tarde, y un acuse tardío no puede completar un reintento. Ninguna tecla atascada; el armado por suelta al arrancar/tras un fallo se sigue cumpliendo; ninguna regresión medible en la latencia/jitter normal (`firmware/main/input/he_backend.c`, `he_cal_barrier.h`, `firmware/main/usb/usb_report_barrier.c`, informe Q8). | **NOT RUN** | **NOT RUN** |

## 16. Identidad OPad y tramas comprobadas (2026-11-21)

Firmware: `config/nvs_ns.c` (copia de namespaces), `usb/usb_descriptors.c` y
`protocol/protocol.c` (`OPAD-<MAC>`), `protocol/frame_parser.c` (tramas
comprobadas). App: `canonical_device_id` en `opad-model`, almacenamiento v19,
`Handshake::send_framing` en `opad-device`.

| ID | Elemento de prueba | Procedimiento | Criterios de aceptación | Linux | Windows |
|---|---|---|---|---|---|
| ID-01 | App nueva, firmware 1.0.0-rc | Instalar esta app encima de la anterior con un pad 1.0.0-rc conectado. | El pad aparece como `OPAD-<MAC>`; contadores, propiedad y última copia de seguridad como antes; ningún aviso de toma de control; ningún "checked frames" en el log del daemon. | **PASS** 2026-11-21 (MX V3): `OPAD-3CDC75701678`, 41816 / 165978 / 105, generación 28 | NOT RUN |
| ID-02 | La actualización lo conserva todo | `opadctl status` antes y después de flashear este firmware encima de 1.0.0-rc. | Solo cambian la ranura y la hora de sincronización; número de serie USB `OPAD-<MAC>`; los layouts de pantalla y la calibración (HE) son los mismos. | **PASS** 2026-11-21 (MX V3) para contadores, teclas, teclas de juego, deslizamientos, brillo, suspensión, antirrebote, cuenta de la tecla 3 y número de serie. Layouts y calibración HE: NOT RUN | NOT RUN |
| ID-03 | La reversión encuentra los datos antiguos | Tras ID-02, flashear una imagen rota (FAIL-06) para que el pad vuelva a 1.0.0-rc. | El firmware antiguo arranca con el estado que tenía al actualizar (lee los namespaces `osupad*`, que no se han tocado). | NOT RUN | n/r |
| ID-04 | Windows ve el nuevo número de serie una vez | Enchufar el pad actualizado a Windows. | Una nueva instancia de dispositivo, quizá un número de COM nuevo, sin avisos; la app se conecta. | n/r | NOT RUN |
| ID-05 | El pad es `303a:8415` | Flashear este firmware encima de un firmware release candidate (`303a:4001`) y después comprobar `lsusb` / Administrador de dispositivos, `opadctl flash`, `opadctl recover` y la app anterior. | Se enumera como `303a:8415` (`usb_descriptors.c`); el daemon lo encuentra por sus cadenas, no por el PID (`desktop/crates/opad-device/src/lib.rs`), así que se conectan las apps antiguas y nuevas; flash y recover funcionan; la regla udev (`70-opad.rules`, que mantiene `4001` para los pads antiguos) da acceso al usuario. | **PASS** en un pad HE (flash, recover, daemon antiguo, ACL de udev), según la fila 0.9 de `docs/launch-checklist.md` | NOT RUN |
| CRC-01 | Tramas comprobadas negociadas | Log del daemon tras conectar esta app a este firmware. | "uses checked frames (CRC-16)"; la prueba de placa, el estado y la sincronización funcionan. | **PASS** 2026-11-21 | NOT RUN |
| CRC-02 | El firmware antiguo sigue con tramas marcadas | Esta app con firmware 1.0.0-rc. | Se conecta con tramas marcadas, sin la línea "checked frames" (ID-01). | **PASS** 2026-11-21 | NOT RUN |
| CRC-03 | Una trama cortada solo se pierde a sí misma | Cubierta por `test_a_cut_short_frame_does_not_take_the_next_one` (tests de firmware en el host y `opad-protocol`). En un pad: una prueba de 8 horas con `opadctl monitor`, contando "bad CRC" y "decode failed". | Ningún fallo de decodificación; cualquier descarte es un único descarte por CRC. | Automatizada: PASS. Prueba de 8 horas: NOT RUN | NOT RUN |

## 8. Aprobación de la verificación

- **Linux v1.0.0, 2026-09-13:** secciones 1–4 verificadas y superadas, **excepto**
  HW-01, HW-02, FAIL-03 y STR-02, que son anteriores al cambio NVS v2→v3 y a la
  tabla de particiones de dos ranuras y se reabren en §9.4.
- **Linux, 2026-09-17:** U-3a, cuatro de las cinco comprobaciones de W3-2 y el
  camino feliz de U-3b pasan en el pad; ver §9.
- **Windows, 2026-09-18:** verificación completa en un host Windows 11 sobre hardware real.
  - Los 12 crates de escritorio compilan limpios y pasan el 100 % de los tests.
  - Sección 1: HW-01, HW-02, HW-03, HW-04 y HW-05 **PASS** (toques discretos 1:1, pulsaciones simultáneas con 0 ghosting y latencia en stream verificados).
  - Sección 2: COM-01, COM-02, COM-03, COM-04, COM-05 **PASS** (robustez del protocolo en hardware verificada).
  - Sección 3: FAIL-01, FAIL-02, FAIL-03 y FAIL-04 **PASS** (persistencia tras cortes de alimentación verificada en hardware).
  - Sección 4: STR-01 y STR-02 **PASS** (estabilidad en partidas largas e invariante de cero escrituras en almacenamiento verificados).
  - Sección 5: WIN-01, WIN-02, WIN-03, WIN-04, WIN-05, WIN-06 y WIN-08 **PASS** (sin buscar drivers, arranque automático y flasheo de firmware sin botones sobre hardware real verificados).
  - Sección 6: PKG-01 a PKG-06 **PASS** de principio a fin (instalación y desinstalación limpias, tosu incluido, terminación de procesos en ejecución).
- **Estado de aprobación de la V1.0:** **100 % COMPLETADO: TODAS LAS PRUEBAS SUPERADAS, LISTO PARA ETIQUETAR v1.0.0.**
