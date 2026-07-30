-- Quién registró la historia y quién la firmó (security-review 2026-07-30).
--
-- # El problema: un solo campo para dos cosas distintas
--
-- `historia.medico_id` decía «a quién se atribuye el acto clínico» y llegaba en
-- el cuerpo de la petición. Con eso, cualquier usuario de la institución podía
-- crear una historia a nombre de un colega y CERRARLA con `firmar`, que pone una
-- firma criptográfica sobre el contenido. El artefacto resultante afirmaba algo
-- que nadie había comprobado: que ese profesional atendió a ese paciente.
--
-- Y `firmar` no registraba al firmante: `UPDATE historia SET firmada=1, firma=?`
-- y nada más. Una firma que no dice quién firmó no acredita a nadie.
--
-- # La corrección: son TRES datos, no uno
--
--   medico_id    a quién se ATRIBUYE el acto. Sigue viniendo del cuerpo —ya
--                validado contra la institución en la 0023— porque un auxiliar
--                que transcribe la nota del médico es una operación real de
--                consultorio, y obligar a que coincidan la prohibiría.
--   creado_por   quién la REGISTRÓ. Lo pone el servidor desde la sesión; el
--                cliente no lo manda ni puede cambiarlo.
--   firmado_por  quién la FIRMÓ, y cuándo. Igual: del servidor, y va DENTRO de
--                lo firmado, así que alterarlo rompe la firma.
--
-- Un solo campo obligaba a mentir en alguno de los dos escenarios. Con tres, el
-- registro dice lo que pasó: atribuida a la doctora, escrita por el auxiliar,
-- firmada por la doctora — o las tres la misma persona, que es el caso corriente
-- y no exige configurar nada.
--
-- NULL en las tres es legítimo y significa «no consta», no «cualquiera»: son
-- filas anteriores a esta migración. No se rellenan con un valor inventado.

ALTER TABLE historia ADD COLUMN creado_por  INTEGER REFERENCES usuario(id);
ALTER TABLE historia ADD COLUMN firmado_por INTEGER REFERENCES usuario(id);
ALTER TABLE historia ADD COLUMN firmado_en  TEXT;
