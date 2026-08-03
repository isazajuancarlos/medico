-- El documento de un profesional no se repite (P11/P12).
--
-- Rescatado de la rama `feat/rips-documentos` (PR #28) al cerrarla: aquel 0037
-- y el 0037 que quedó en `main` crearon las mismas dos columnas, pero el de allá
-- traía además esta restricción y se perdió al quedarse con el otro. Va en un
-- archivo nuevo porque `sqlx` guarda el hash de cada migración: editar una ya
-- aplicada rompe el arranque contra una base existente.
--
-- PARCIAL, y no es laxitud. La mayoría de las filas van a tener el documento en
-- NULL durante mucho tiempo —lo estrenamos hoy, y los usuarios que ya existen no
-- lo traen—, y en SQL dos NULL no colisionan; sin el `WHERE`, el índice sería
-- correcto igual, pero decir explícitamente qué filas cubre es lo que impide que
-- el día que alguien lo lea crea que la columna es obligatoria.
--
-- Es GLOBAL, no por institución, y ahí sí hay una decisión: una persona es la
-- misma persona en dos IPS, y un mismo documento en dos filas distintas
-- significa o que alguien se equivocó al teclear o que hay dos cuentas para el
-- mismo profesional. Las dos cosas hay que verlas, no repartirlas por sede.
--
-- Lo que este índice NO hace es comprobar que el tipo sea uno de los ocho que
-- la 948 admite. Eso vive en `medico_dominio::muv::admite_como_profesional`,
-- que se GENERA del maestro del validador; un CHECK aquí sería una segunda
-- lista escrita a mano —lo que este proyecto no hace con ningún catálogo— y
-- además exigiría reconstruir `usuario`, a la que apuntan 25 claves foráneas,
-- dentro de la transacción en que `sqlx` envuelve cada migración.
CREATE UNIQUE INDEX ux_usuario_documento
    ON usuario (tipo_documento, numero_documento)
    WHERE tipo_documento IS NOT NULL AND numero_documento IS NOT NULL;
