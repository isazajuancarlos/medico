-- Firma criptográfica de la historia clínica (hallazgo A2 de la auditoría de
-- seguridad). Hasta ahora `firmada` era un booleano: quien escribiera en la base
-- podía voltearlo o alterar el contenido de una historia firmada sin dejar
-- rastro. `firma` guarda una firma post-cuántica (Ed25519+ML-DSA-87, vía
-- guaca/Quipu) sobre el contenido canónico de la historia; verificarla detecta
-- cualquier cambio posterior.
--
-- NULL cuando no hay clave de firma configurada: la firma es opcional y su
-- ausencia nunca impide firmar (cerrar) la historia — la misma postura de
-- «desactivar con gracia» que en informes.

ALTER TABLE historia ADD COLUMN firma TEXT;
