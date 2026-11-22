---
title: Guía de Desarrollo
description: Flujo de trabajo de colaboradores, ciclo de vida de ramas git, firma de commits, trailers de atribución de IA y pautas de arquitectura del repositorio.
category: overview
source_files:
  - /AGENTS.md
  - /README.md
  - /.github/workflows/ci.yml
  - /Makefile
---

Te damos la bienvenida a la documentación de desarrollo de OPad. Esta sección cubre los flujos de trabajo de desarrollo, las invariantes del repositorio, las instrucciones de compilación, los entornos de prueba y las referencias de arquitectura para contribuir al firmware, a la suite de escritorio y a los diseños de hardware. <!-- src: /AGENTS.md -->

## Core Product Invariants

Todo el desarrollo en este repositorio debe adherirse a las reglas inmutables de producto definidas en [AGENTS.md](file:///home/paella/Documents/projects/esp32/osu-pad-esp32-docs/AGENTS.md): <!-- src: /AGENTS.md -->

1. **La Latencia Siempre Gana**: Si una característica empeora de forma medible la latencia de las teclas o la fluctuación (jitter), se reduce, se pospone, se congela durante el juego o se elimina. Nada se une a la ruta crítica de teclas: la pantalla, el almacenamiento, la bandeja del sistema y tosu permanecen completamente fuera de ella.
2. **El Pad es un Teclado sin Software Instalado**: Funciona como un teclado compuesto USB HID a 1000 Hz que no requiere controladores ni daemons en Linux ni en Windows.
3. **La Aplicación y el Pad se Actualizan de Forma Independiente**: Los protocolos de comunicación deben seguir siendo bidireccionales (aplicación nueva con firmware antiguo, aplicación antigua con firmware nuevo). Nunca se debe exigir actualizar ambos juntos.
4. **La Pantalla Táctil es el Tercer Botón**: En partida, el dedo en reposo es Quick Retry (`` ` ``) y los deslizamientos nunca lo activan. Fuera de partida, los deslizamientos ejecutan acciones multimedia. Los deslizamientos en plena partida nunca insertan Alt en el informe del teclado.
5. **Nunca los Botones BOOT o RESET**: El flasheo, la actualización y la recuperación ocurren estrictamente por USB o serie. Una actualización flashea la ranura OTA alterna y revierte si la enumeración USB falla.
6. **Cada Tecla Proviene de un Dedo**: Nada de macros, pulsación automática, entradas programadas o temporizadas, ni comandos de inyección de teclas del host al pad.

## Git Workflow & Repository Rules

Los colaboradores deben seguir las siguientes reglas de flujo de trabajo en git: <!-- src: /AGENTS.md -->

- **Historial Lineal**: Rebase sobre `main`. Sin commits de fusión (merge commits).
- **Ramas de Vida Corta**: Las ramas existen solo mientras el trabajo está activo. Elimina las ramas y los worktrees inmediatamente tras la fusión.
- **Identidad de Autor y Declaración de IA**:
  - Firma los commits con tu propio nombre y correo electrónico. Nunca pongas una herramienta de IA como autor del commit.
  - Incluye un trailer de declaración de IA al final del mensaje de commit:
    - `AI: assisted`: Una persona dirigió o escribió el cambio con asistencia de IA.
    - `AI: generated`: Una IA escribió el cambio, revisado y confirmado por una persona.
    - La ausencia de trailer indica que no hubo participación de IA.
- **Commits Convencionales**: Da formato a los asuntos de los commits como `type(scope): subject (#issue)`.

## Code as the Single Source of Truth

La documentación en `docs/` se redacta contra el código base: <!-- src: /AGENTS.md -->
- Cuando el código y la documentación discrepan, **el código es la autoridad**. Corrige o actualiza la documentación para que coincida con el código.
- Cada afirmación factual en la documentación de desarrollo incluye citas a las fuentes (comentarios HTML invisibles a la fuente o enlaces directos a archivos). <!-- src: /AGENTS.md -->
- Nunca inventes parámetros, opciones de CLI ni pines de hardware que no estén presentes en el código base.

## Documentation Structure (Diátaxis Framework)

La documentación de OPad sigue los cuatro cuadrantes de Diátaxis:
- **Tutoriales (`docs/es/start/*`)**: Guías paso a paso para recién llegados (Qué Necesitas, Primera Conexión, Cableado de las Teclas).
- **Guías Paso a Paso (`docs/es/guides/*`)**: Instrucciones orientadas a tareas (Actualizar Firmware, Calibrar Hall Effect, Copia de Seguridad y Restauración).
- **Explicación (`docs/es/explanation/*`)**: Contexto conceptual y justificación de arquitectura (Arquitectura del Sistema, Ingeniería de Latencia, La Pantalla Táctil como Tercer Botón).
- **Referencia (`docs/es/reference/*` y `docs/es/develop/*`)**: Catálogos detallados de APIs, comandos de CLI, ajustes del dispositivo, distribución de pines e información interna de subsistemas.
