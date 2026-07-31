-- DIVIPOLA: los municipios y departamentos de Colombia (#16/#77).
--
-- Hasta ahora `paciente.cod_municipio` era TEXT sin validación ninguna. En el
-- backend Python que se retira sí había un validador, y con él un conocimiento
-- que se habría perdido al borrarlo. Esto lo trae al lado Rust.
--
-- EL CERO A LA IZQUIERDA NO ES COSMÉTICO. `codMunicipioResidencia` va de CINCO
-- dígitos. Antioquia es el 05 y Atlántico el 08: **148 municipios, el 13 % del
-- país, tienen código que empieza por cero**, Medellín entre ellos. El conjunto
-- de MinSalud los publica SIN ese cero (`5001` en vez de `05001`) porque en
-- algún punto pasaron por una hoja de cálculo que los leyó como números.
-- Cargar de ahí sin normalizar deja mal reportado a todo paciente de esos dos
-- departamentos. De ahí el CHECK de longitud: aquí no entra un código de cuatro.
--
-- LAS DOS FUENTES OFICIALES NO COINCIDEN, Y NO ES UN ERROR:
--     27086 Belén de Bajirá (MinSalud)  <->  27493 Nuevo Belén de Bajirá (DANE)
--     94663 Mapiripana (MinSalud)       <->  94885 La Guadalupe (DANE)
-- La primera es el pleito de límites entre Chocó y Antioquia; la segunda, un
-- área no municipalizada del Guainía. Se aceptan LOS CUATRO códigos: rechazar
-- los de MinSalud rompe el envío al MUV, que valida contra su propia lista, y
-- rechazar los del DANE rompe el cruce con cualquier estadística oficial. Un
-- prestador no tiene por qué arbitrar un diferendo territorial para facturar.
-- `en_minsalud` distingue cuál es cuál sin excluir a ninguno.

CREATE TABLE IF NOT EXISTS departamento (
    codigo TEXT PRIMARY KEY CHECK (length(codigo) = 2),
    nombre TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS municipio (
    codigo         TEXT PRIMARY KEY CHECK (length(codigo) = 5),
    nombre         TEXT NOT NULL,
    departamento   TEXT NOT NULL REFERENCES departamento(codigo),
    -- «Municipio», «Isla», «Área no municipalizada»… tal como lo publica la
    -- fuente. No se normaliza: la categoría territorial es un dato del DANE,
    -- no una etiqueta nuestra.
    tipo           TEXT NOT NULL,
    -- Está en la lista de MinSalud, que es contra la que valida el MUV.
    en_minsalud    INTEGER NOT NULL CHECK (en_minsalud IN (0, 1))
);

CREATE INDEX IF NOT EXISTS ix_municipio_departamento ON municipio (departamento);
CREATE INDEX IF NOT EXISTS ix_municipio_nombre ON municipio (nombre);
