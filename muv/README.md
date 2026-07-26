# Validador FEV-RIPS en local

El contenedor contra el que habla `backend/muv_client.py`. **Ambiente de
pruebas**, siempre.

## De dónde sale la imagen

MinSalud **sí publica la imagen**, en un Azure Container Registry, y entrega el
`docker compose` con la credencial de extracción dentro:

| Archivo | Registro e imagen | Perfil |
|---|---|---|
| `apilocal-dockercompose.Stage.yml` | `crmspsgovcostage.azurecr.io/stage-fevrips-apilocal` | `DockerStage` |
| `apilocal-dockercompose.Production.yml` | `crmspsgovcoprd.azurecr.io/production-fevrips-apilocal` | `DockerProduction` |

Comprobado el 2026-07-26 contra el registro real: la credencial del archivo de
stage autentica, y `stage-fevrips-apilocal:latest` existe — 8 capas, 120 MB
comprimidos, digest `sha256:add9b16b16e7df51…`.

**La *Guía de Instalación API Docker FEV-RIPS v1.0* describe un método
anterior**, en el que uno construía las imágenes copiando dentro un
`FevsRipsLocal.bak`, un `FEVRIPS-script.sql` y una carpeta `FVE1/`. Nada de eso
hace falta ya: el API crea y migra su propio esquema. Queda dicho porque esa
guía sigue publicada y manda por un camino que ya no es.

Los dos `.yml` **no se versionan** (`.gitignore`): llevan dentro la credencial
del registro, la clave del `sa` y la del certificado. El repositorio es privado,
pero eso protege de quien no tiene acceso, no de un `git add -A`.

## Nunca el de producción

`apilocal-dockercompose.Production.yml` apunta al validador con **efectos
jurídicos y financieros reales**. Una factura inventada obtendría allí un CUV
verdadero. Para probar, solo el de stage.

Ojo con las copias editadas a mano: se encontró una en `reporips/api/` que hace
`docker login` contra el registro de *stage* pero pide la imagen de
*producción*. Comprobar siempre que el registro del `login`, el del `image:` y
el `ASPNETCORE_ENVIRONMENT` digan los tres lo mismo.

## Levantar

```bash
cd muv
export CERT_PASSWORD='...'     # la que quieras; no hay defecto
./certificado.sh               # una sola vez

# La credencial NO está en este repositorio: sale del .yml del Ministerio.
docker login -u puller -p <la del archivo de stage> crmspsgovcostage.azurecr.io

docker compose pull
docker compose up -d
```

Y después, desde `backend/`:

```bash
set -a; . ~/.config/medico/muv.entorno; set +a
python validar_muv.py --enviar --sin-verificar-tls
```

`--sin-verificar-tls` es correcto y no una concesión: el certificado lo genera
`certificado.sh` y es autofirmado, pero la conexión no sale de la máquina.

## Lo que se cambió del archivo del Ministerio, y por qué

1. **`C:\Certificates` → `./certificates`.** Ruta de Windows; en Linux docker la
   tomaría como nombre de volumen y el API arrancaría sin certificado.
2. **El `.pfx` se genera aquí.** El archivo espera un `fevripsapilocal.pfx` que
   no viene con él. `certificado.sh` lo fabrica y comprueba que abre con la
   clave; sin `CERT_PASSWORD` se niega, en vez de dejar una clave de ejemplo
   copiada por todo el mundo.
3. **`MSSQL_PID=Enterprise` → `Developer`.** Funcionalmente equivalente para
   esto, y su licencia cubre desarrollo y pruebas sin coste. Declarar Enterprise
   afirma una licencia que no tenemos, y quien acepta el EULA es quien corre el
   contenedor.
4. **La base persiste en un volumen.** El original no guarda nada: recrear el
   contenedor borraba el esquema recién migrado.
5. **Los límites de CPU y memoria no se copiaron.** El propio archivo trae la
   nota «validar documentación referente, para los recursos del límite y reserva
   del contenedor» — es decir, ni el Ministerio los da por buenos. Sin ellos
   docker no restringe nada, que en una máquina de desarrollo es lo que se
   quiere.

## Sin verificar

- Si la clave del `sa` (`FEVRIPS1*`, fijada por el Ministerio) puede cambiarse,
  o si el API la lleva horneada en su cadena de conexión. Mientras no se sepa,
  se deja como está; lo que la protege es que el 1433 no se publica al host.
- Que el perfil `DockerStage` apunte de verdad a `stage-fevrips.sispro.gov.co`.
  Se confirma leyendo la configuración dentro de la imagen, ya descargada.
