---
title: Laboratorio de Pruebas Automatizadas
description: Descripción general del laboratorio con máquinas virtuales multi-OS autoalojado, aislamiento del ejecutor, pruebas automatizadas de instalación, validación de actualizaciones y verificación de desinstalación.
category: develop
type: reference
---

# Laboratorio de Pruebas Automatizadas

Para garantizar que los instaladores, las rutas de actualización y las integraciones de escritorio funcionen impecablemente en diversos sistemas operativos, OPad opera un laboratorio de pruebas físico, autoalojado y dedicado.

Este documento describe la arquitectura del laboratorio, las restricciones de seguridad, la topología de las máquinas virtuales y los procedimientos de pruebas automatizadas.

<!-- src: /scripts/lab/lab-common.ps1#AllVms -->
<!-- src: /.github/workflows/lab-check.yml#opad-lab -->
<!-- src: /.github/workflows/lab-install.yml#concurrency -->
<!-- src: /scripts/lab/vm-install-linux.sh#installed_version -->

---

## 1. Hardware Architecture and Security Model

El laboratorio se ejecuta en un mini PC dedicado con Windows 11 registrado como un ejecutor autoalojado de GitHub Actions con las etiquetas `[self-hosted, opad-lab]`.

```mermaid
flowchart TD
    GH["Orquestador de GitHub Actions\n(release-app.yml o disparo manual)"] -->|Se ejecuta en [self-hosted, opad-lab]| MiniPC["Host Mini PC Windows 11\n(Oracle VirtualBox 7.x)"]
    MiniPC --> Snap["Revertir VM a Snapshot Limpio 'limpia'"]
    Snap --> Boot["Arrancar Máquinas Virtuales en Modo Headless"]
    Boot --> VM1["Ubuntu 22.04 / 24.04 / 26.04 (APT)"]
    Boot --> VM2["Fedora 43 (DNF)"]
    Boot --> VM3["Arch Linux (AUR opad-bin)"]
    Boot --> VM4["Windows 10 / Windows 11 (Inno Setup)"]
    VM1 --> Test["Ejecutar vm-install-linux.sh vía SSH"]
    VM2 --> Test
    VM3 --> Test
    VM4 --> WinTest["Ejecutar vm-install-windows.ps1 vía SSH"]
```

### Security Isolation Invariant

> [!IMPORTANT]
> **Sin Ejecución en Pull Requests**: El ejecutor autoalojado **nunca** ejecuta código proveniente de pull requests o forks. Solo se dispara en flujos de trabajo oficiales de release ([.github/workflows/lab-install.yml](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/.github/workflows/lab-install.yml)) o flujos disparados manualmente ([.github/workflows/lab-check.yml](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/.github/workflows/lab-check.yml)) en la rama `main`.

---

## 2. Target VM Topology

El laboratorio mantiene siete máquinas virtuales gestionadas mediante VirtualBox ([scripts/lab/lab-common.ps1](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/scripts/lab/lab-common.ps1#L11-L19)):

| Identificador VM | SO Huésped | Formato de Paquete | Puerto SSH del Host | Gestor de Paquetes |
|---|---|---|---|---|
| `ubuntu-22.04` | Ubuntu 22.04 LTS | `.deb` | 2222 | `apt-get` |
| `ubuntu-24.04` | Ubuntu 24.04 LTS | `.deb` | 2224 | `apt-get` |
| `ubuntu-26.04` | Ubuntu 26.04 LTS | `.deb` | 2226 | `apt-get` |
| `fedora-43` | Fedora 43 | `.rpm` | 2243 | `dnf` |
| `arch` | Arch Linux | AUR `opad-bin` | 2250 | `pacman` / `makepkg` |
| `windows-10` | Windows 10 Pro | `.exe` | 2310 | Inno Setup |
| `windows-11` | Windows 11 Pro | `.exe` | 2311 | Inno Setup |

Cada máquina virtual arranca con inicio de sesión automático en una sesión de escritorio `opad` sin privilegios y proporciona `sudo` sin contraseña (Linux) o privilegios de Administrador (Windows) a través de puertos SSH redirigidos en bucle local (loopback).

---

## 3. Automated Installation and Upgrade Lifecycle

El entorno de pruebas principal está orquestado por [scripts/lab/lab-install.ps1](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/scripts/lab/lab-install.ps1), que ejecuta una prueba de tres fases en cada VM:

### Phase 1: Clean Baseline Installation
1. El host revierte la VM a su instantánea limpia `limpia` usando `VBoxManage snapshot restore`.
2. La VM arranca hasta que su entorno gráfico de escritorio está activo (Linux: sesión en `seat0`; Windows: sesión activa de consola).
3. El paquete de la release estable previa se transfiere vía SCP y se instala.
4. La VM se reinicia para verificar que los servicios de inicio automático se inicien correctamente.

### Phase 2: In-Place Upgrade Verification
1. El paquete candidato a release nuevo se transfiere e instala.
2. La VM se reinicia en la sesión de escritorio.
3. Script de verificación ([scripts/lab/vm-install-linux.sh](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/scripts/lab/vm-install-linux.sh)):
   - Verifica que la versión binaria instalada coincida con el tag de la release.
   - Afirma que todos los archivos instalados coincidan con las sumas de verificación de la release.
   - Valida que `opad-daemon` y la GUI en la bandeja del sistema estén ejecutándose.
   - Comprueba la comunicación con el daemon mediante `opadctl status`.
   - Confirma que `tosu` haya conservado su capacidad de seguridad `cap_sys_ptrace=eip`.
   - Confirma que los ajustes de usuario y el historial en la base de datos (`~/.local/share/opad/opad.db` o `%LOCALAPPDATA%\opad\opad.db`) hayan sobrevivido a la actualización intactos.

### Phase 3: Clean Uninstallation
1. El paquete se desinstala mediante el gestor de paquetes de la distribución o el desinstalador de Windows.
2. La prueba verifica la eliminación completa:
   - Todos los binarios instalados en `/usr/bin/` o `Program Files` se han eliminado.
   - Los archivos `.desktop` de inicio automático y las unidades de servicio de systemd son eliminados.
   - Las reglas udev se eliminan y recargan.
   - No quedan procesos huérfanos ejecutándose en segundo plano.

---

## 4. Health Checks and Diagnostics

El estado del laboratorio puede verificarse bajo demanda mediante [.github/workflows/lab-check.yml](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/.github/workflows/lab-check.yml):
- Verifica que la sesión del host Windows 11 esté en ejecución.
- Arranca todas las máquinas virtuales configuradas a partir de instantáneas en paralelo o individualmente.
- Captura capturas de pantalla automáticas del escritorio de cada huésped para detectar regresiones en la interfaz gráfica, almacenándolas como artefactos del flujo de trabajo durante 14 días.
- Publica resúmenes de ejecución usando `scripts/lab/lab-results.ps1`.
