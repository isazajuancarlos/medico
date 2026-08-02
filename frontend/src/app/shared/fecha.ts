// SPDX-License-Identifier: LicenseRef-Propietario

/**
 * `2026-08-02` en la zona del navegador.
 *
 * El día LOCAL, no el UTC. El calendario de Material entrega la medianoche
 * local, y `toISOString().slice(0, 10)` —que es lo que había en todas partes—
 * la traduce a UTC: en Colombia (UTC-5) sale el mismo día y no se nota, pero al
 * este de Greenwich sale el ANTERIOR. Una licencia que vence un día antes de lo
 * pactado, o una cita que se agenda en la víspera, no falla en ningún sitio: se
 * descubre tarde y no hay forma de reproducirlo desde Medellín.
 */
export function diaLocal(d: Date): string {
  const mm = String(d.getMonth() + 1).padStart(2, '0');
  const dd = String(d.getDate()).padStart(2, '0');
  return `${d.getFullYear()}-${mm}-${dd}`;
}
