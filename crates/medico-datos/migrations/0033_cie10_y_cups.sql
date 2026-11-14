-- Los maestros clínicos: CIE-10 (diagnósticos) y CUPS (procedimientos) — #77.
--
-- Hasta ahora `historia.diagnostico_principal` y `historia.cod_consulta_cups`
-- eran TEXT libre. El RIPS los valida contra las tablas del MUV, así que un
-- código mal digitado tiene la forma correcta, pasa cualquier validación de
-- estructura, y hace rechazar la factura entera tres semanas después. Tenerlos
-- aquí convierte eso en una línea del informe de cierre.
--
-- Se siembran desde JSON empotrado en el binario, igual que DIVIPOLA y por lo
-- mismo: el despliegue en sitio es un binario y nada más, y un consultorio rural
-- no puede depender de que haya internet para saber si un diagnóstico existe.
-- Medido: parsear los dos JSON cuesta 46 ms y sembrar las 26 063 filas, 37 ms en
-- una sola transacción. No hace falta ningún mecanismo de «sembrar solo si
-- cambió»: sería un segundo camino para estar al día, y el primero ya funciona.

-- LA DAGA Y EL ASTERISCO. 378 códigos del CIE-10 acaban en `†` o `*`. No es
-- corrupción: es la notación daga-asterisco de la clasificación, donde `†` marca
-- la etiología y `*` la manifestación. Pero el MUV valida contra su tabla y
-- `A065†` no está en ninguna, así que el código se guarda LIMPIO y la notación
-- va en su propia columna — no se pierde, y no contamina el código.
--
-- Comprobado antes de decidirlo: los 12 423 códigos son distintos, y siguen
-- siendo distintos tras quitar la notación. CERO colisiones, así que limpiar no
-- funde dos diagnósticos en uno.
CREATE TABLE IF NOT EXISTS cie10 (
    codigo      TEXT PRIMARY KEY CHECK (length(codigo) BETWEEN 3 AND 4),
    descripcion TEXT NOT NULL,
    -- Los tres primeros caracteres: la categoría a la que pertenece. Se guarda
    -- derivada y no se recalcula al consultar, porque los informes
    -- epidemiológicos agrupan por ella en cada corrida.
    categoria   TEXT NOT NULL,
    -- '', 'daga' o 'asterisco'. Vacío es la mayoría.
    notacion    TEXT NOT NULL DEFAULT ''
);

CREATE INDEX IF NOT EXISTS ix_cie10_categoria ON cie10 (categoria);
CREATE INDEX IF NOT EXISTS ix_cie10_descripcion ON cie10 (descripcion);

-- CUPS: Clasificación Única de Procedimientos en Salud.
--
-- `seccion` se guarda TAL CUAL viene de la fuente, incluidas las dos filas cuyo
-- texto está corrompido (`SECCIO4R 00 …` y `NULL`). Corregirlas a ojo sería
-- inventar a qué sección pertenece un procedimiento; que salgan en el informe y
-- las arregle quien tenga la fuente buena.
CREATE TABLE IF NOT EXISTS cups (
    codigo     TEXT PRIMARY KEY,
    nombre     TEXT NOT NULL,
    seccion    TEXT NOT NULL,
    -- EN QUÉ ARCHIVO DEL RIPS se reporta: AP (procedimientos), AC (consultas) o
    -- AT (otros servicios). Sale de la columna que la fuente titula
    -- «Habilitado» y que NO lleva un sí/no — se descubrió mirando los valores en
    -- vez del nombre. Es justo la pregunta que el generador de RIPS tiene que
    -- hacerse por cada procedimiento, así que se guarda en vez de deducirla.
    ambito_rips TEXT NOT NULL CHECK (ambito_rips IN ('AP','AC','AT')),
    quirurgico INTEGER NOT NULL CHECK (quirurgico IN (0, 1))
);

CREATE INDEX IF NOT EXISTS ix_cups_nombre ON cups (nombre);
CREATE INDEX IF NOT EXISTS ix_cups_seccion ON cups (seccion);
