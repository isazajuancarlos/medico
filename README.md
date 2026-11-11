# medico

**A health-information system for Colombian healthcare providers** — clinical
records, patient care, billing, and the full **RIPS → FEV-RIPS → MUV** chain of
Resolution 948 of 2026.

**English** · [Español](#español)

---

## What it is

`medico` is a working information system for health service providers (IPS) in
Colombia. It covers the clinical and administrative core — patient history, care
encounters, billing — and takes the regulatory reporting chain seriously:
building valid RIPS, producing the FEV-RIPS invoice, and getting through MUV
validation **before** a rejection costs a provider time and money.

It is not a from-scratch experiment. It reimplements, on a modern stack, a system
that is in production today — with the compliance edge cases that only show up
after real use.

## Who it's for

Small and mid-sized IPS that need their records, billing and MinSalud reporting
to line up, without a legacy desktop application and without a team of
integrators.

## Data protection

Patient data is personal data. `medico` is built around Colombia's **Ley 1581**
and its decrees — purpose limitation, retention and suppression, and an auditable
trail — as a design constraint, not an afterthought.

## This repository

This is the public home of the project. `medico` is a **commercial product**; its
source is not published here. To see it in action or discuss a deployment,
**visit [xiliux.com](https://xiliux.com)**.

---

## Español

**Sistema de información para prestadores de servicios de salud en Colombia** —
historia clínica, atención, facturación y la cadena completa
**RIPS → FEV-RIPS → MUV** de la Resolución 948 de 2026.

### Qué es

`medico` es un sistema de información para IPS en Colombia. Cubre el núcleo
clínico y administrativo —historia del paciente, atención, facturación— y se toma
en serio la cadena de reporte regulatorio: construir un RIPS válido, producir el
FEV-RIPS y pasar la validación del MUV **antes** de que un rechazo le cueste
tiempo y dinero al prestador.

No es un experimento de cero: reimplementa, sobre un stack moderno, un sistema que
hoy está en producción —con los casos borde de cumplimiento que solo aparecen tras
el uso real—.

### Para quién

IPS pequeñas y medianas que necesitan que su historia clínica, su facturación y su
reporte a MinSalud cuadren, sin una aplicación de escritorio heredada y sin un
equipo de integradores.

### Protección de datos

Los datos del paciente son datos personales. `medico` se construye en torno a la
**Ley 1581** de Colombia y sus decretos —finalidad, retención y supresión, y una
traza auditable— como restricción de diseño, no como añadido.

### Este repositorio

Es la casa pública del proyecto. `medico` es un **producto comercial**; su código
no se publica aquí. Para verlo en funcionamiento o conversar un despliegue,
**visita [xiliux.com](https://xiliux.com)**.

---

© 2026 Juan Carlos Isaza. Proprietary — all rights reserved.
