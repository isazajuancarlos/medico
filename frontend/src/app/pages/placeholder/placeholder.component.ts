import { Component, Input, OnInit, inject, ChangeDetectionStrategy } from '@angular/core';
import { ActivatedRoute } from '@angular/router';
import { MensajeComponent } from '../../shared/ui/mensaje.component';

@Component({
  selector: 'app-placeholder',
  standalone: true,
  imports: [MensajeComponent],
  changeDetection: ChangeDetectionStrategy.Eager,
  template: `
    <div class="page-card">
      <div style="display: flex; align-items: center; gap: 0.75rem; margin-bottom: 1.5rem;">
        <i class="pi pi-tools" style="font-size: 1.5rem; color: #3b82f6;"></i>
        <h2 class="page-title" style="margin-bottom: 0">{{ displayTitle }}</h2>
      </div>
      <x-mensaje severity="info" text="Módulo en desarrollo — disponible próximamente." />
    </div>
  `,
})
export class PlaceholderComponent implements OnInit {
  @Input() title = '';
  private route = inject(ActivatedRoute);
  displayTitle = 'Módulo';

  ngOnInit(): void {
    this.displayTitle = this.title || this.route.snapshot.data['title'] || 'Módulo';
  }
}
