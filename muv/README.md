# Validador FEV-RIPS en local

Los dos contenedores contra los que habla `backend/muv_client.py`. La receta
está escrita; **faltan los artefactos**, que no son públicos.

## Lo que hay que conseguir

MinSalud **no publica una imagen**: la *Guía de Instalación API Docker FEV-RIPS
v1.0* no contiene ningún `docker pull`. El prestador descarga un paquete y
construye las imágenes con él.

**De dónde sale.** Del micrositio de Factura Electrónica de Venta en Salud:

> https://www.sispro.gov.co/central-financiamiento/Pages/facturacion-electronica.aspx

Los *Lineamientos para la generación, validación y envío de RIPS como soporte de
la FEV* (v3.2, MinSalud) dicen que allí está «la versión vigente de estos
lineamientos y de los documentos y **herramientas técnicas** emitidos por el
Ministerio». Es la misma página desde la que se descarga el validador
cliente-servidor.

Esa página no se puede consultar con `curl`: devuelve un 302 hacia sí misma en
bucle, así que hay que abrirla en un navegador. Por lo mismo **no está
verificado** si la descarga es pública o exige sesión de SISPRO. Lo que sí está
confirmado (caso 2026403888) es que *usar* el MUV exige credenciales de una
persona vinculada a la entidad; descargar el paquete puede ser otra cosa.

**Si el prestador ya factura**, el camino corto es su propia máquina: la carpeta
con la que construyó su contenedor tiene los tres archivos.

**Mesa de ayuda** (capítulo 7.1 de los Lineamientos):
`soporte-fev-rips@minsalud.gov.co` · Bogotá (601) 330 5043 · resto del país
018000960020 · L-V 7:00–18:00, sábados 8:00–13:00.

Del paquete salen tres cosas, que van aquí:

| Archivo del paquete | Dónde ponerlo |
|---|---|
| `FevsRipsLocal.bak` | `sqlserver/artefactos/` |
| `FEVRIPS-script.sql` | `sqlserver/artefactos/scripts/` |
| carpeta `FVE1/` (con `FVE.WebAPI.dll`) | `api/FVE1/` |

Basta con el `.bak` **o** con el `.sql`: el respaldo trae la base entera y el
script la construye vacía. Si están los dos se usa el respaldo. Si no está
ninguno, la construcción falla en vez de dejar un SQL Server vacío que
reventaría después como un error incomprensible del API.

Ninguno de los tres se versiona: son de MinSalud, el respaldo pesa, y no nos
corresponde redistribuirlos. Están en `.gitignore`.

## Levantar

```bash
export MSSQL_SA_PASSWORD='...'          # sin valor por defecto, a propósito
cd muv && docker compose up --build     # la primera vez tarda: restaura la BD
```

Y después, desde `backend/`:

```bash
set -a; . ~/.config/medico/muv.entorno; set +a
python validar_muv.py --enviar --sin-verificar-tls
```

`--sin-verificar-tls` solo es defendible porque la conexión no sale de la
máquina: el certificado del contenedor es autofirmado.

## Lo que cambia respecto de la guía, y por qué

La guía es un borrador publicado —conserva el código de proceso sin rellenar
(`XXXSXX`) y frases de plantilla— y va en v1.0 mientras el manual de consumo de
la API va en v2.0 para la v4.3. Cinco diferencias deliberadas:

1. **La clave del `sa` no se hornea.** La guía la pone con `ENV` en el
   Dockerfile; ahí queda en una capa y `docker history` la enseña. Va por
   entorno, y el arranque se cae si falta.
2. **`sqlcmd` se busca en vez de suponerse.** La guía fija
   `/opt/mssql-tools/bin/sqlcmd`. Comprobado dentro de la imagen real: **esa
   ruta no existe**. Lo único que hay es `/opt/mssql-tools18/bin/sqlcmd`, que
   además exige `-C`. Seguir la guía literalmente da un «no such file» en cada
   consulta, que no se parece al problema real.
3. **No se restaura dos veces.** Una marca en el volumen de datos evita que
   recrear el contenedor vuelva a restaurar con `REPLACE` sobre lo ya validado.
4. **La cadena de conexión va por entorno.** La del manual apunta a `localhost`
   —que dentro del contenedor del API es el propio API— y separa el puerto con
   dos puntos en vez de coma.
5. **El 1433 no se publica al host.** Los contenedores se hablan por la red
   interna de compose.

## Lo que está sin verificar

Cómo sirve TLS el API en el 9443: dev-cert, certificado montado o HTTP simple.
Depende de lo que traiga el paquete y se ajusta al abrirlo. Está marcado como
tal en `compose.yml`.
