# API REST del validador FEV-RIPS.
# Guía de Instalación API Docker FEV-RIPS v1.0 (MinSalud), sección 11.4.
#
# .NET 7 dejó de recibir soporte en mayo de 2024. Se conserva porque es lo que
# fija la guía y lo que presumiblemente targetea FVE.WebAPI.dll: la imagen base
# tiene que coincidir con el runtime contra el que se compiló el ensamblado, no
# con la que preferiríamos. Si el paquete que entregue el Ministerio trae una
# DLL más nueva, subir esta etiqueta es el primer ajuste.
FROM mcr.microsoft.com/dotnet/aspnet:7.0

WORKDIR /app

# La guía hace aquí un `RUN apt-get update` suelto. No se copia: sin un
# `install` detrás no instala nada, invalida la caché en cada construcción y
# deja las listas de paquetes dentro de la imagen.

# La carpeta de publicación la entrega el Ministerio. Ver ../README.md.
COPY FVE1/ .

ENTRYPOINT ["dotnet", "FVE.WebAPI.dll"]
