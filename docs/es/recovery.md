---
title: Conciliación de contadores, recuperación y desvinculación
description: Reglas de conciliación de contadores, procedimientos de flasheo, entrada en el bootloader y recuperación de un teclado no enumerado.
category: how-to
source_files:
  - /desktop/cli/src/main.rs
  - /desktop/crates/opad-device/src/flash.rs
  - /desktop/daemon/src/sync.rs
  - /desktop/daemon/src/identity.rs
  - /firmware/main/usb/usb_cdc.c
  - /firmware/main/config/device_config.c
  - /firmware/main/app_main.c
  - /firmware/main/protocol/protocol.c
  - /firmware/main/runtime/runtime.c
---

## 1. Dual-Storage Model (§12, §13)

Tanto el ESP32-S3 como el PC host mantienen contadores de pulsaciones de por vida para ambas teclas físicas:
- **Dispositivo (ESP32-S3)**: Los contadores se incrementan en RAM directamente en la ruta crítica de procesamiento de teclas en el Núcleo 0. **No se realiza ninguna escritura en flash durante las pulsaciones.** Los contadores solo se guardan en NVS durante el estado `IDLE` una vez concluido el enfriamiento posterior al mapa.
- **PC host**: Se almacenan en SQLite (tabla `device_state`). **No se realiza ninguna escritura en disco durante `PLAYING` ni `COOLDOWN`.** El demonio mantiene los valores más recientes en memoria y los confirma en SQLite al pasar a `SYNC/IDLE`.

---

## 2. Counter Generation & Monotonic Reconciliation

Cada registro de contador contiene:
- `device_id`: Identificador único de hardware derivado de la dirección MAC del ESP32-S3 (`OPAD-<MAC>`).
- `counter_generation`: Entero de incremento monótono. Se aumenta cada vez que los contadores se restablecen intencionadamente o se restauran desde una copia de seguridad.
- `lifetime_key1`: Pulsaciones totales acumuladas para la Tecla 1.
- `lifetime_key2`: Pulsaciones totales acumuladas para la Tecla 2.

### Reconciliation Rules (§13):
1. **Precedencia generacional**: Si `generation_A > generation_B`, el estado A prevalece incondicionalmente. Esto evita que una copia de seguridad antigua o una base de datos desactualizada sobrescriba un restablecimiento deliberado.
2. **Fusión monótona (misma generación)**: Cuando las generaciones coinciden, se conserva el valor máximo de cada tecla:
   $$\text{Key1} = \max(\text{pc.k1}, \text{esp.k1}), \quad \text{Key2} = \max(\text{pc.k2}, \text{esp.k2})$$
3. **Política ante pérdida de energía (§12.1)**: Si la alimentación se interrumpe bruscamente durante una partida activa antes de concluir el enfriamiento, pueden perderse las pulsaciones no guardadas desde el último punto de control en IDLE. Se trata de un compromiso de diseño intencionado para garantizar un procesamiento de teclas sin latencia sin bloqueos por escrituras en flash.

---

## 3. Connect-Time Reconciliation Matrix

Cuando el demonio abre el puerto serie USB CDC y recibe `HelloAck`:

| Condición | Acción tomada |
|---|---|
| **Teclado conocido, misma generación** | El demonio fusiona los recuentos más altos ($\max(\text{pc}, \text{esp})$) y sincroniza tanto la RAM del dispositivo como SQLite. |
| **Teclado conocido, gen del dispositivo > gen del host** | El dispositivo se restauró o restableció en otro equipo. El host actualiza su base de datos SQLite para coincidir con la generación y recuentos del dispositivo. |
| **Teclado conocido, gen del host > gen del dispositivo** | El dispositivo fue reflasheado o borrado. El host envía `CounterSync` para restaurar los recuentos guardados en el dispositivo. |
| **Teclado nuevo, estado en blanco (`gen == 0`, `k1 == 0`, `k2 == 0`)** | Si existe exactamente un teclado anterior en SQLite, se activa el **Flujo de sustitución de dispositivo**. De lo contrario, se inicializa el nuevo dispositivo en la generación 1. |

---

## 4. Device Replacement Flow (§14 / P1-5)

Cuando un jugador sustituye su OPad o cambia de hardware:
1. El demonio detecta una conexión procedente de un `device_id` desconocido, pero encuentra exactamente un registro de dispositivo previo en SQLite.
2. El demonio crea una recomendación de sustitución pendiente (`suggested_replacement: Some(old_device_id)`).
3. La GUI muestra un aviso no bloqueante:
   > *"Nuevo teclado detectado (`OPAD-NEW`). ¿Transferir las pulsaciones acumuladas de tu teclado anterior (`OPAD-OLD`)?"*
4. **Si el usuario acepta (`InheritDeviceCounters`)**:
   - El demonio incrementa `counter_generation` en 1.
   - Migra los recuentos acumulados anteriores al nuevo `device_id` en SQLite.
   - Transmite `CounterSync(force_restore = true)` al nuevo teclado, programando las estadísticas previas en el hardware.
5. **Si el usuario lo rechaza o lo descarta**:
   - El nuevo teclado inicia su ciclo de vida limpiamente con su propio `device_id`, generación 1 y 0 pulsaciones.
   - El registro del dispositivo anterior se conserva en SQLite como dato histórico.

---

## 5. Backup, Restore, and Reset Actions

- **Restablecer contadores**:
  - Restablece a 0 las pulsaciones acumuladas de la Tecla 1 y la Tecla 2.
  - Incrementa automáticamente `counter_generation` para que todos los pares reconozcan el restablecimiento como la versión más reciente.
  - Sincroniza tanto la base SQLite del host como la NVS del dispositivo.
- **Exportar copia de seguridad**:
  - Exporta un documento JSON portátil según la especificación del §21 que contiene:
    - Versión del formato (`format_version: 1`).
    - Metadatos de hardware (`device_id`, `board_profile`, `counter_generation`).
    - Estadísticas (`lifetime_key1`, `lifetime_key2`).
    - Configuración (`key1`, `key2`, `debounce_us`, `brightness`, `display_sleep_seconds`, `gameplay_display_hz`).
- **Importar copia de seguridad (`PreviewImport` e `ImportBackup`)**:
  - Control de seguridad en dos fases:
    1. `PreviewImport`: Valida el esquema JSON, las asignaciones de teclas y los contadores. Devuelve una comparativa entre el estado actual y el de la copia.
    2. `ImportBackup`: Requiere confirmación explícita (`confirm: true`). Conserva el `tosu_endpoint` local, incrementa la generación, aplica la configuración al dispositivo y guarda el nuevo estado de contadores.

### 5.1 Automatic backups

Exportar manualmente solo ayuda a quienes se acuerdan de hacerlo, por lo que el demonio también genera una copia de seguridad **de forma automática**, en el mismo formato JSON del §21, tras cada sesión de juego:

| | |
|---|---|
| Dónde | `<data>/backups/opad-backup-YYYYMMDDTHHMMSSZ.json` — `~/.local/share/opad/backups` en Linux, `%APPDATA%\opad\backups` en Windows |
| Cuándo | **20 segundos** después de que una sesión se estabilice en `IDLE` |
| Cuántas | Las **10 más recientes**. Las más antiguas se eliminan; los archivos no generados por el demonio nunca se tocan. |
| Restaurar | `opadctl import <archivo>` — idéntico a una exportación manual, ya que es el mismo documento creado por el mismo código |
| Última copia | `opadctl status` → `Last Backup`, y la ruta completa se registra en el log del demonio cada vez que se escribe una |

Los 20 segundos no son por capricho. Una copia de seguridad es una escritura de almacenamiento, y la regla P1-3 prohíbe escrituras durante `PLAYING` y `COOLDOWN` (§6). La secuencia tras un mapa es `COOLDOWN` → `SYNC` → `IDLE`; el temporizador comienza solo una vez que la sincronización posterior a la partida ha concluido, y la escritura vuelve a comprobar que el estado sigue siendo `IDLE` antes de efectuarse. Iniciar otro mapa dentro de esa ventana **cancela** la escritura pendiente en lugar de aplazarla; la siguiente sesión iniciará una nueva.

Además, está diseñada de forma deliberada para no causar problemas: si una copia de seguridad no se puede escribir, se registra en el log y se ignora. Nunca provoca el fallo de la sincronización, nunca bloquea el demonio y nunca muestra un diálogo de error, ya que una red de seguridad que rompe lo que protege es peor que no tenerla. Como corolario, conviene revisar `Last Backup` periódicamente en lugar de darlo por sentado.

Lo que **no** hace: es una copia de los contadores, no una segunda fuente de verdad. Se escribe después de la sincronización, de modo que si esta falló, la copia contendrá los mismos datos desactualizados del demonio: mejor que nada, que es el objetivo, pero no sustituye a una sincronización funcional.

---

## 6. Deferred Operations During Gameplay

Durante los estados `PLAYING` y `COOLDOWN`:
- Todas las escrituras en SQLite y en la flash del ESP32-S3 quedan estrictamente bloqueadas.
- Si el usuario cambia la configuración, reinicia el diseño o fuerza una sincronización mientras juega, el demonio encola la acción en RAM.
- Cuando tosu reporta `IDLE` (o tras 5 segundos de enfriamiento posterior al mapa), el supervisor de ejecución aplica de manera segura todas las operaciones pendientes en NVS y en disco.

---

## 7. Reflashing & Unbinding a Pad (§W3-4)

Esta es la **única** forma de desvincular un teclado. No existe un botón para desemparejar en la aplicación ni un gesto de restablecimiento de fábrica en el teclado deliberadamente, ya que una asignación de propiedad que pudiera eliminarse por cable no protegería la integridad de los contadores (§W3-2 rechaza una reclamación con ceros por la misma razón).

Si has llegado aquí desde el aviso de toma de posesión: **no** necesitas nada de esto para utilizar el teclado. "Dejarlo como está" ya lo mantiene funcionando como teclado. Esta sección sirve para transferir el teclado a otra persona o para devolver a un estado conocido un teclado que no arranca.

### 7.1 What it costs

El registro del propietario (`owner_id`) reside en la partición NVS del teclado, junto con los contadores de pulsaciones acumuladas. Nada puede borrar uno y conservar el otro.

> **`espflash erase-flash` borra los contadores de vida útil del lado del ESP.**
> La Tecla 1 y la Tecla 2 vuelven a cero en el teclado. También se borran la configuración del dispositivo y los diseños guardados.

Borra la totalidad de los 16 MB —la aplicación, el bootloader y la tabla de particiones también—, por lo que el teclado deja de funcionar como teclado hasta que el paso 7.4 vuelva a cargar el firmware.

**No queda inutilizado (brickeado), y no es posible que lo quede.** El bootloader de primera etapa del ESP32-S3 reside en la ROM de máscara y ningún comando del host puede borrarlo. Un teclado con la flash totalmente vacía continúa enumerándose como `303a:1001` y aceptando flasheos.

### 7.2 Export first

Tu host mantiene su propia copia de los contadores en SQLite, pero exporta el archivo JSON portátil de todos modos: es la única copia que sobrevive a reinstalar la aplicación:

```bash
opadctl export ~/opad-backup.json     # o bien: en la app → Dispositivo → Exportar copia
```

Es probable que ya exista una. Desde que se implementaron las copias automáticas (§5.1), el demonio genera una exportación JSON 20 segundos después de cada sesión de juego, manteniendo las diez más recientes en `<data>/backups/`. Revisa qué hay guardado antes de borrar nada:

```bash
opadctl status | grep 'Last Backup'
ls -t ~/.local/share/opad/backups/ | head        # %APPDATA%\opad\backups en Windows
```

Exporta manualmente de todas formas. Solo lleva un segundo, guarda el archivo en una ubicación elegida por ti en vez de un directorio que una reinstalación podría limpiar, y la copia automática tiene la antigüedad de tu última sesión, que no necesariamente coincide con la de los contadores si has jugado desde la última sincronización.

Restaúrala después con `opadctl import ~/opad-backup.json`, lo que incrementa la generación del contador y devuelve los recuentos al teclado (§5).

**Un detalle importante antes de borrar.** Reflashear deja el teclado sin reclamar, y un teclado sin reclamar es reclamado *silenciosamente* por el primer host al que se conecte (§W3-3). Si ese host es aquel desde el que acabas de borrarlo, restaurará también los contadores que recuerda, porque su generación será superior a la del teclado en blanco (§3). Eso es lo ideal si estás reparando una instalación dañada, pero un error si vas a ceder el teclado: en ese caso, conéctalo primero al ordenador de su nuevo propietario para que sea su instalación la que lo reclame.

### 7.3 Put the pad in download mode and free the port

```bash
opadctl bootloader
```

Esto ejecuta ambos pasos: solicita a `opad-daemon` que libere el puerto serie y luego reinicia el teclado en el bootloader de descarga ROM de forma desatendida, mostrando el puerto en el que reaparece. Funciona incluso si el demonio no se está ejecutando.

**Windows: el puerto debe estar libre, no solo inactivo.** Los identificadores serie en Windows son exclusivos (`CreateFileW` sin modo compartido), por lo que cualquier proceso que mantenga abierto el puerto hará que el flasheo falle de inmediato en vez de solo ralentizarlo, a diferencia de Linux, donde un segundo lector es solo descortés. `opadctl bootloader` gestiona el demonio; cierra por tu cuenta cualquier monitor serie, ventana de PuTTY o del IDE de Arduino. Si el flasheo se detiene con una infracción de uso compartido o "acceso denegado", ese es el motivo.

Localiza el puerto con `espflash list-ports` o en Administrador de dispositivos → Puertos (COM y LPT). Los puertos superiores a COM9 son válidos; tanto `opadctl` como `espflash` aplican el prefijo `\\.\` requerido por los nombres COM estándar.

**La peculiaridad del reinicio en descarga USB.** El teclado se reinicia a sí mismo en modo de descarga a través de su interfaz USB-Serial-JTAG, y dicha interfaz conserva la dirección USB que tenía al encenderse. El host debe detectar una desconexión real antes de la reconexión, por lo que el firmware fuerza una; si no surte efecto, se produce el error `-71` y la aplicación vuelve a arrancar. Por este motivo, `opadctl bootloader` intenta tres activadores distintos. Si todos fallan, recurre a `opadctl recover` (§7.7): introduce el teclado en modo de descarga mediante una reconexión del cable, sin intervención del firmware. El teclado nunca requiere pulsar sus botones BOOT o RESET.

### 7.4 Erase, then flash

Con el teclado en modo de descarga:

**Linux**

```bash
espflash erase-flash -p /dev/ttyACM0 --before no-reset --after no-reset-no-stub
opadctl flash --full ~/Downloads/opad-1.0.0/          # o firmware/build
```

**Windows (PowerShell o cmd)**

```powershell
espflash erase-flash -p COM3 --before no-reset --after no-reset-no-stub
opadctl flash --full C:\Users\tu\Downloads\opad-1.0.0\
```

`--before no-reset` es fundamental: el teclado ya está en modo de descarga, y permitir que espflash intente su propio reinicio DTR/RTS por USB-Serial-JTAG solo corre el riesgo de devolverlo a la aplicación. Borrar los 16 MB lleva cerca de un minuto. **No lo desconectes mientras se ejecuta.**

`--full` es obligatorio en este paso, dado que `erase-flash` eliminó también el bootloader y la tabla de particiones. Escribe las cuatro imágenes en orden —bootloader en `0x0`, tabla de particiones en `0x8000`, datos OTA en `0xf000` y aplicación en `0x20000` (`ota_0`, §U-3a)— y luego reinicia el teclado en la aplicación. Acepta tanto un directorio de release desempaquetado como el árbol `firmware/build` de ESP-IDF.

### 7.5 Confirm

```bash
opadctl status
```

Un teclado que se ha reiniciado correctamente indicará `ESP32 Device: Connected` y un `Running Slot` con valor `ota_0`.

Los contadores dependerán del host al que lo hayas conectado. En un equipo que nunca haya visto este teclado, se esperan cero pulsaciones en la generación 1. En el equipo desde el que lo borraste, se reflejarán los recuentos recordados: su generación supera a la del teclado en blanco, por lo que los restaura y vuelve a reclamar el teclado de forma silenciosa, y el log del demonio confirmará ambos eventos (§7.2).

Si el teclado no vuelve a iniciar la aplicación en 15 segundos, `opadctl flash` lo indicará. Repite **únicamente el paso de flasheo del 7.4**; no vuelvas a borrar la memoria. El teclado continúa en modo de descarga y volver a flashear es completamente seguro. Si ha salido del modo de descarga y tampoco se detecta como OPad, utiliza `opadctl recover` (§7.7).

### 7.6 When flashing is interrupted

Una actualización de la aplicación (`opadctl flash <app.bin>` o la actualización de firmware desde la app) escribe en la ranura en la que el teclado **no** se está ejecutando, y solo entonces hace que el bootloader apunte a ella (`plan_app_update` en `desktop/crates/opad-device/src/flash.rs`). La imagen en ejecución no se modifica, de modo que:

- **Interrupción antes del final:** la selección de arranque aún no se había escrito y el teclado arranca la imagen previa.
- **Escrita, pero la nueva imagen está dañada:** arranca una vez en modo de prueba. El firmware solo la confirma una vez que el host ha configurado la interfaz USB (`confirm_image_once_usb_works` en `firmware/main/app_main.c`). Si el host reinicia el bus y no lo configura en 20 segundos, el teclado se reinicia y el bootloader marca la imagen como abortada, arrancando la versión anterior. Un bucle de reinicios revierte el sistema exactamente de la misma manera. `opadctl flash` informará de que "no se aceptó el nuevo firmware y se volvió al anterior"; el teclado funcionará como antes.

Un flasheo `--full` (§7.4) escribe `ota_0` directamente sin alternativa de respaldo. Si se interrumpe, el teclado **no** queda inservible (§7.1): ejecuta `opadctl recover` (§7.7).

### 7.7 A pad that does not show up as OPad (`opadctl recover`)

Cuando no es posible comunicarse con la aplicación por USB en absoluto (por ejemplo, con una compilación que arranca pero nunca se enumera: `can't set config #1, error -32` en `dmesg`), nada puede ordenarle que acceda al bootloader. Recupéralo desconectando y volviendo a conectar el cable:

```bash
opadctl recover firmware/build         # o un directorio de release desempaquetado
```

y desconecta y vuelve a conectar el cable USB del teclado cuando la herramienta lo indique. En cada encendido, antes de que la aplicación tome el control del puerto USB, la ROM del ESP32-S3 aparece en el bus como `303a:1001` durante una fracción de segundo. `recover` aprovecha esa ventana, reinicia el chip en modo de descarga a través de USB (las líneas DTR/RTS de USB-Serial-JTAG), desactiva el watchdog que el bootloader había iniciado, escribe el conjunto completo como hace `--full` y lo inicia (`recover` en `desktop/crates/opad-device/src/flash.rs`). En ningún momento se requiere pulsar los botones BOOT o RESET.

Un reinicio por software no vuelve a activar el puerto de la ROM; únicamente lo hace una conexión eléctrica real, motivo por el cual es lo único que se requiere hacer físicamente.
