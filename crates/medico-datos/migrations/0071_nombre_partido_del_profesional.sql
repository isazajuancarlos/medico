-- El nombre PARTIDO del profesional, que el RDA de encuentro (#603 fase 4) exige
-- y que `usuario` no tenía. El `Practitioner` del RDA lleva `name` con `given`
-- (nombres) y `family` (apellidos) por separado, igual que el `Patient`; el
-- paciente ya tiene `nombre1/nombre2/apellido1/apellido2` desde la fundación, y
-- el profesional se había quedado con un solo campo `nombre` (texto completo).
--
-- El RIPS nunca lo necesitó —reporta al profesional SOLO por documento (P11/P12),
-- nunca por nombre—, así que el dato no existía en ninguna parte estructurada. Un
-- nombre completo no se puede partir sin adivinar dónde acaba el nombre y empieza
-- el apellido (directiva 20), y en el visor nacional `given`/`family` invertidos
-- no dan error: presentan a «Pérez Juan» como si «Pérez» fuera el nombre.
--
-- Todos NULL para los usuarios actuales: es un dato que hoy ninguna pantalla
-- captura, así que nacen ausentes y el productor del RDA los anota como FALTA en
-- vez de emitir un Practitioner a medias. La captura en la pantalla de usuarios
-- es un paso aparte; el `nombre` completo se queda como está para el resto del
-- sistema (login, listados, quién firmó).
ALTER TABLE usuario ADD COLUMN nombre1   TEXT;
ALTER TABLE usuario ADD COLUMN nombre2   TEXT;
ALTER TABLE usuario ADD COLUMN apellido1 TEXT;
ALTER TABLE usuario ADD COLUMN apellido2 TEXT;
