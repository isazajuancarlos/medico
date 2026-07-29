-- Cifrado en reposo de las imágenes diagnósticas (hallazgo A1). Los archivos se
-- guardaban en claro en el disco; un backup robado o el acceso al sistema de
-- ficheros del VPS los leía directamente (dato sensible de salud, Ley 1581).
--
-- `cifrada` marca si el archivo en disco está cifrado (blob de guaca/Quipu) o en
-- claro. Convive con lo ya subido: 0 = archivo tal cual (histórico o sin clave),
-- 1 = hay que descifrarlo con la clave de reposo para servirlo. Así encender el
-- cifrado no invalida lo anterior.

ALTER TABLE imagen_diagnostica ADD COLUMN cifrada INTEGER NOT NULL DEFAULT 0;
