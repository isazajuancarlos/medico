-- Antecedentes clínicos del paciente. Son cuatro bloques de estructura abierta
-- (cada consultorio y cada especialidad registran cosas distintas), así que van
-- como JSON y no como columnas fijas: forzar un esquema rígido aquí obligaría a
-- migrar la base cada vez que una IPS añade un campo a su ficha.
--
-- Default '{}' —objeto vacío, JSON válido— para que un paciente sin antecedentes
-- registrados no traiga NULL: quien lee siempre recibe un objeto, nunca la
-- ausencia disfrazada.

ALTER TABLE paciente ADD COLUMN ant_familiares    TEXT NOT NULL DEFAULT '{}';
ALTER TABLE paciente ADD COLUMN ant_personales    TEXT NOT NULL DEFAULT '{}';
ALTER TABLE paciente ADD COLUMN ant_ginecologicos TEXT NOT NULL DEFAULT '{}';
ALTER TABLE paciente ADD COLUMN ant_obstetricos   TEXT NOT NULL DEFAULT '{}';
