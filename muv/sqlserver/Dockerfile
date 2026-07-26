# Base de datos del validador FEV-RIPS.
# Guía de Instalación API Docker FEV-RIPS v1.0 (MinSalud), sección 11.3.
FROM mcr.microsoft.com/mssql/server:2022-latest

# ACCEPT_EULA sí va aquí: es una constante, no un secreto.
# SA_PASSWORD NO va aquí, aunque la guía lo ponga: `ENV` queda escrito en la
# capa y `docker history` lo enseña. Se pasa por entorno al arrancar.
ENV ACCEPT_EULA=Y

WORKDIR /usr/src/app

# Los tres artefactos los entrega el Ministerio al prestador; no están en este
# repositorio ni son públicos. Ver ../README.md.
COPY artefactos/ ./
COPY entrypoint.sh .

USER root
RUN chmod +x entrypoint.sh
USER mssql

CMD ["./entrypoint.sh"]
