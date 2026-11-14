-- Una cirugía SUSPENDIDA vuelve a la vida por `Iniciar` (reanudar), y hasta hoy
-- eso pisaba `hora_inicio_real` con la hora del reinicio, perdiendo la del primer
-- inicio sin dejar rastro (#537). Son dos datos, no uno (directiva 32): la hora
-- NOMINAL del primer inicio mide cuándo empezó de verdad la atención, y la del
-- reinicio mide la reanudación. Se separa la segunda en su propia columna.
--
-- Nullable y sin defecto: la inmensa mayoría de cirugías nunca se suspenden, así
-- que `hora_reinicio` en NULL es «no se reanudó», que es la verdad — no un cero
-- ni un centinela.

ALTER TABLE programacion_quirurgica ADD COLUMN hora_reinicio TEXT;
