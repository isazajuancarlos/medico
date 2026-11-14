-- El SEGUNDO cupo de la licencia: cuántos MÉDICOS admite el plan.
--
-- Un valor con dos significados son dos campos (directiva 32), y aquí eran dos
-- magnitudes distintas colapsadas en una: «cuánta gente cabe» y «cuántos de esos
-- pueden atender». No se cambian una por otra —un plan de 20 usuarios con 3
-- médicos es un producto, y uno de 20 usuarios con 20 médicos es otro— así que
-- una sola cota deja al sistema ciego a la que factura.
--
-- Lo tenía el sistema Python vivo (`core/deps.py:337`,
-- `hay_cupo(..., para_medico=True)` contra `licencia.max_medicos`) y se perdió
-- al portar: medido el 2026-08-06, una IPS con plan de 3 médicos podía dar de
-- alta 20 sin que nada se quejara.
--
-- ─── NULL NO ES CERO, Y NO ES UN DEFECTO ───
--
-- La columna es ANULABLE y sin `DEFAULT`, a propósito (directiva 20). Las
-- licencias que ya existen se firmaron sin este campo: no sabemos cuántos
-- médicos contrató cada cliente, y ponerle un número —el que sea— sería
-- inventar una cláusula de un contrato firmado. `NULL` significa exactamente
-- «este plan no pactó cupo de médicos», y el código lo trata como lo que es:
-- no hay segunda cota que comprobar. Cero significaría «no puede tener
-- ninguno», que es lo contrario y dejaría a un prestador vivo sin poder dar de
-- alta a nadie que atienda.
--
-- El `CHECK` admite el NULL y exige que, si hay número, sea positivo: un plan
-- de cero médicos no es un plan inusual, es uno imposible (directiva 21).

ALTER TABLE licencia ADD COLUMN max_medicos INTEGER
    CHECK (max_medicos IS NULL OR max_medicos > 0);
