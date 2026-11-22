---
title: Arquitectura de Hardware y Ecosistema de PCBs
description: Historia de generaciones de hardware (V1, V2, V3), placas carrier, módulos de entrada intercambiables MX y Hall Effect, carcasas impresas en 3D personalizadas y suite de verificación de hardware opadverify.
---

# Arquitectura de Hardware y Ecosistema de PCBs

OPad separa el controlador anfitrión —una placa Waveshare ESP32-S3-Touch-LCD-2— de los interruptores físicos de teclas utilizando una arquitectura de módulos de entrada intercambiables.

Este documento detalla las iteraciones de PCB (de V1 a V3), la distribución de pines de los conectores, la mecánica de los cables, los parámetros de las carcasas personalizadas y la suite de verificación automatizada (`opadverify`).

<!-- src: /hardware/pcb/V3/README.md#OPad V3 PCBs -->
<!-- src: /hardware/pcb/V3/README.md#ADS7028 -->
<!-- src: /hardware/pcb/V3/README.md#LP5907 -->
<!-- src: /hardware/pcb/V1/README.md#OPad V1 PCBs -->
<!-- src: /hardware/pcb/V1/README.md#DRV5055A3 -->
<!-- src: /hardware/pcb/V2/README.md#OPad V2 PCBs -->
<!-- src: /hardware/pcb/V2/README.md#ADS7038 -->
<!-- src: /hardware/3d/README.md#Waveshare ESP32-S3-Touch-LCD-2 -->
<!-- src: /hardware/3d/README.md#opad_enclosure.scad -->
<!-- src: /hardware/verify/README.md#opadverify -->
<!-- src: /hardware/verify/README.md#test_system.py -->

---

## 1. Hardware Generations Overview

| Generación | Placa Carrier | Tipo de Cable | Módulos de Teclas | Arquitectura de ADC y Sensores | Estado |
|---|---|---|---|---|---|
| **V1** | Carrier de controlador V1 (en forma de C, cabeceras P1/P2) | JST SH de 8 pines (1.0 mm) | MX V1 (2 teclas)<br>HE V1.1 (2 teclas) | HE V1.1 enrutaba voltajes analógicos desde sensores TI DRV5055A3 por cable al ADC1 interno del ESP32-S3. | Reemplazado |
| **V2** | Carrier de controlador V1 | JST SH de 8 pines (1.0 mm) | Prototipo HE V2 (3 teclas) | Digitalización en el módulo mediante ADC SPI de 12 bits TI ADS7038. Promediado interno. | Prototipo |
| **V3** | Carrier de controlador V3 + Placa USB V3 | FFC de 14 vías y 1.0 mm (Módulo)<br>FFC de 8 vías y 1.0 mm (USB) | MX V3 (3 teclas)<br>HE V3 (3 teclas) | ADC SPI de 12 bits TI ADS7028 en el módulo, LDO de ultra bajo ruido LP5907 alimentado por 5 V VBUS, zonas de exclusión de masa para sensores. | **Producción Actual** |

---

## 2. V3 Production PCB Ecosystem

La generación V3 ([hardware/pcb/V3/README.md](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/hardware/pcb/V3/README.md)) resuelve el ruido analógico y la tensión mecánica introduciendo una placa USB dedicada y cables planos flexibles (FFC) de alta densidad:

```mermaid
flowchart LR
    USB_C["Puerto USB-C\n(Pared Trasera)"] -->|FFC de 8 vías\n(Alimentación + USB D+/D-)| USB_PCB["Placa USB V3"]
    USB_PCB --> Carrier["Carrier de Controlador V3"]
    Carrier -->|Zócalos de Cabecera| ESP["Waveshare ESP32-S3-Touch-LCD-2\n(Controlador y Pantalla)"]
    Carrier -->|FFC de 14 vías\n(SPI / Teclas / ID / Alim)| Module["Módulo de Entrada V3\n(MX V3 o HE V3, 3 Teclas)"]
```

### Key Differences in V3

1. **Placa USB-C Dedicada**: Alivia la tensión mecánica por inserción del conector integrado en la placa de Waveshare. Contiene diodos de protección ESD, perlas de ferrita en VBUS, condensadores de filtrado y un diodo Schottky que evita la alimentación inversa.
2. **Cable de Módulo FFC de 14 Vías Reversible**: Sustituye el frágil conector JST SH por un FFC de paso de 1.0 mm tipo B.
3. **Digitalización en el Propio Módulo (HE V3)**: Incorpora un ADC SAR de 12 bits TI **ADS7028** conectado por SPI. Las señales analógicas de los sensores permanecen estrictamente en la placa del módulo.
4. **Alimentación Aislada para Sensores**: Los sensores Hall Effect y el voltaje de referencia del ADS7028 se alimentan mediante un regulador de baja caída (LDO) **LP5907** integrado conectado a los 5 V de VBUS, evitando completamente el ruido del convertidor reductor conmutado de Waveshare.
5. **Ensamblaje a Una Sola Cara**: Todos los componentes de montaje superficial residen en la cara inferior de los módulos y en la superior del carrier, reduciendo los costes de fabricación en JLCPCB.

---

## 3. Cable Pinouts and Wiring

### Module Cable (14-Way FFC, 1.0 mm Pitch, Type B)

El FFC de 14 vías conecta el Carrier V3 con el módulo MX V3 o HE V3:

| Contacto | Red Carrier | Red HE V3 | Red MX V3 | Notas |
|---|---|---|---|---|
| **1** | GND | GND | GND | Masa |
| **2** | 5V | 5V | – | Alimentación VBUS para el LDO LP5907 |
| **3** | 3V3 | 3V3 | 3V3 | Suministro de nivel lógico |
| **4** | GPIO8 | Divisor ID | Divisor ID | Voltaje de ID de módulo (ADC1_CH7) |
| **5** | GPIO6 | SPI MISO | KEY3 | Entrada digital / SPI |
| **6** | GND | GND | GND | Blindaje de masa |
| **7** | GPIO2 | SPI SCLK | – | Reloj SPI (ADC1_CH1) |
| **8** | GPIO4 | SPI MOSI | – | SPI MOSI (ADC1_CH3) |
| **9** | GND | GND | GND | Blindaje de masa |
| **10** | GPIO10 | SPI CS | KEY1 | Selección de chip / Key 1 (ADC1_CH9) |
| **11** | GPIO7 | – | KEY2 | Key 2 (ADC1_CH6) |
| **12** | 3V3 | 3V3 | 3V3 | Suministro de nivel lógico |
| **13** | 5V | 5V | – | Alimentación VBUS |
| **14** | GND | GND | GND | Masa |

> [!NOTE]
> **Reversibilidad del Cable**: Si el cable de 14 vías se inserta invertido (conectando el contacto $k$ al $15 - k$), la alimentación y la masa permanecen asignadas a alimentación y masa. La línea de ID del módulo se intercambia entre GPIO8 y GPIO7. El firmware muestrea ambos canales ADC en el arranque y reasigna automáticamente la matriz de GPIO.

#### Module ID Voltage Detection

Cada módulo de entrada contiene un divisor de resistencias en la línea de ID muestreado por el ADC del controlador en el arranque:

| Tipo de Módulo | Divisor de Resistencias ($R_{sup} / R_{inf}$) | Voltaje Nominal ID | Rango de Umbral en Firmware |
|---|---|---|---|
| **MX V1** | 100 kΩ / 10 kΩ | 0.30 V | $0.00\,\text{V} - 0.68\,\text{V}$ |
| **HE V1.1** | 100 kΩ / 47 kΩ | 1.06 V | $0.68\,\text{V} - 1.35\,\text{V}$ |
| **HE V3 (y V2)**| 100 kΩ / 100 kΩ | 1.65 V | $1.35\,\text{V} - 1.95\,\text{V}$ |
| **MX V3** | 47 kΩ / 100 kΩ | 2.24 V | $1.95\,\text{V} - 2.60\,\text{V}$ |
| **Sin Conectar / Ninguno** | Flotante (pull-down interno) | ~0.00 V | Pasa por defecto a cabeceras cableadas a mano |

### USB Cable (8-Way FFC, 1.0 mm Pitch, Type A)

Conecta la placa USB trasera con la placa carrier:

| Contacto | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
|---|---|---|---|---|---|---|---|---|
| **Red** | GND | 5V | D− | D+ | D+ | D− | 5V | GND |

La distribución de pines es completamente simétrica: el contacto $k$ coincide idénticamente con el $9 - k$. Las líneas USB D+ y D− utilizan conductores emparejados para garantizar la integridad de señal, y los dos pines de 5V transportan de forma segura hasta 1.0 A combinado.

---

## 4. 3D Enclosure Design

La carcasa personalizada está modelada paramétricamente en OpenSCAD ([hardware/3d/README.md](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/hardware/3d/README.md)):

- **Carcasa Superior y Base**: `hardware/3d/custom_case/V3/opad_enclosure.scad`.
- **Dimensiones de la Pantalla**: Se adapta al cristal de la lente de Waveshare (envolvente de $37.52 \times 59.22\,\text{mm}$) y al área activa visible de la pantalla ($30.99 \times 41.20\,\text{mm}$).
- **Placa de Interruptores (Switch Plate)**: Grosor de placa estándar de 1.5 mm con aberturas de $14.0 \times 14.0\,\text{mm}$ y separación estándar entre centros de teclas de $19.05\,\text{mm}$.
- **Ensamblaje**: Se mantiene unida mediante cuatro pasadores de esquina y postes de presión, sin requerir tornillos a través de la PCB de interruptores.

---

## 5. Automated Hardware Verification (opadverify)

Para evitar errores de fabricación (como la huella reflejada de SOT-23 en HE V1.0), OPad utiliza una suite de verificación personalizada ([hardware/verify/README.md](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/hardware/verify/README.md)):

```bash
# Configuración del entorno de verificación
python3 -m venv .tools/venv && .tools/venv/bin/pip install -r hardware/verify/requirements.txt
cd hardware/verify

# Verificar netlist dorada contra los archivos Gerber de fabricación
../../.tools/venv/bin/python -m opadverify golden/carrier_v3.toml
../../.tools/venv/bin/python -m opadverify golden/he_v3.toml

# Ejecutar comprobación de continuidad y orientación del cable
../../.tools/venv/bin/python tests/test_system.py
```

### Verification Checks

1. **Lateralidad de Huellas (Footprint Handedness)**: Reconstruye la conectividad directamente a partir de las geometrías de cobre en Gerber y los taladros metalizados, verificando la lateralidad de pines con las hojas de datos de los fabricantes.
2. **Alineación de CPL en JLCPCB**: Obtiene los modelos exactos de huella EasyEDA de JLCPCB para todos los componentes de la lista de materiales (BOM) y valida que los componentes colocados encajen exactamente en los pads objetivo sin rotación manual.
3. **Simulación de Inversión del Cable**: `test_system.py` ejecuta el trazado de redes de extremo a extremo desde los pads del módulo hasta los GPIO del ESP32 tanto en orientación normal como invertida del cable.
