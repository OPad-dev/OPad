---
title: Uso con tosu
description: Cómo configurar y ejecutar el escáner de memoria integrado tosu para telemetría de beatmaps de osu! en tiempo real, transmisión de UR y requisitos de permisos.
---

OPad integra `tosu` (v4.26.2), un lector de memoria de alto rendimiento y código abierto que examina el proceso en ejecución `osu!.exe` para transmitir estadísticas de juego en vivo a la pantalla LCD del teclado. <!-- src: /desktop/crates/opad-tosu/src/lib.rs -->

## Cómo funciona la integración con tosu

1. `opad-daemon` inicia y supervisa el binario integrado `tosu` en segundo plano (`bundled_tosu_binary`). <!-- src: /desktop/crates/opad-tosu/src/lib.rs -->
2. `tosu` expone un servidor WebSocket local en `ws://127.0.0.1:24050/websocket/v2`. <!-- src: /desktop/crates/opad-tosu/src/lib.rs#DEFAULT_TOSU_ENDPOINT -->
3. El demonio se conecta a la transmisión WebSocket y recibe paquetes JSON en tiempo real con información sobre:
   - Artista de la canción, título, nombre de la dificultad y creador del mapa.
   - Precisión en vivo, combo, número de fallos y calificación actual (SS, S, A, B, C, D).
   - Unstable Rate (UR) y pulsaciones por minuto en vivo (KPM).
4. El demonio transforma las diferencias de telemetría en sobres `GameplayDisplayState` y `DataUpdate`, enviándolos a través de USB CDC a la pantalla del teclado a una tasa limitada entre 1 Hz y 30 Hz (`flush_interval_playing`). <!-- src: /desktop/daemon/src/telemetry.rs#flush_interval_playing -->

## Configuración de permisos en Linux (ptrace)

En las distribuciones modernas de Linux, el módulo de seguridad Yama restringe la inspección de memoria de procesos (`ptrace`) para que los procesos no privilegiados no puedan leer la memoria de otros programas. <!-- src: /packaging/linux/deb/postinst -->

Si instalaste OPad a través de los paquetes `.deb`, `.rpm` o AUR, las capacidades de archivo se configuran automáticamente: <!-- src: /packaging/linux/deb/postinst -->
```bash
sudo setcap cap_sys_ptrace=eip /usr/lib/opad/tosu/tosu
```
Esta capacidad permite a `tosu` acoplarse a `osu!.exe` bajo Wine o Proton nativo sin requerir privilegios de root ni desactivar los controles de seguridad del sistema.

### Solución de problemas con ptrace en instalaciones personalizadas
Si tosu muestra `Waiting for osu!` incluso con el juego en ejecución, verifica la capacidad:
```bash
getcap /usr/lib/opad/tosu/tosu
```
Si aparece vacío, vuelve a aplicarla:
```bash
sudo setcap cap_sys_ptrace=eip $(which tosu || echo /usr/lib/opad/tosu/tosu)
```

## Configuración en Windows

En Windows 10 y 11, `tosu` funciona de forma predeterminada sin necesidad de privilegios especiales.
- Inicia `osu!`.
- Inicia `opad-gui` o asegúrate de que `opad-daemon` esté en ejecución.
- Al acceder a la selección de canciones o comenzar a jugar un mapa, la pantalla LCD del teclado pasa inmediatamente de la pantalla `Idle` a la pantalla `Playing`, mostrando los metadatos de la canción y los medidores de la partida en vivo.

## Comprobar el estado de tosu

Puedes inspeccionar el estado de la conexión en vivo de dos formas:
1. **A través de la GUI**: En `opad-gui`, ve a **Ajustes > Acerca de tosu**. El panel muestra el estado del proceso tosu, la salud de la lectura de memoria y la detección del cliente de osu! activo.
2. **A través de la CLI**:
   ```bash
   opadctl status
   ```
   La salida informará si `tosu` se está ejecutando, si está conectado al punto de conexión WebSocket y si está registrando la partida de forma activa.
