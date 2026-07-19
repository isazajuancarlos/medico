import { Component } from '@angular/core';
import { PlaceholderComponent } from '../placeholder/placeholder.component';

@Component({
  selector: 'app-superadmin',
  standalone: true,
  imports: [PlaceholderComponent],
  template: '<app-placeholder title="SuperAdmin - Gestión Multi-Institucional" />'
})
export class SuperadminComponent {}
