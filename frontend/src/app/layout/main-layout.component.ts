import { Component, ChangeDetectionStrategy } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { SidebarComponent } from './sidebar.component';
import { HeaderComponent } from './header.component';

@Component({
  selector: 'app-main-layout',
  standalone: true,
  // Ya no hay `<p-toast />`: `MatSnackBar` no necesita un elemento en la
  // plantilla —se monta en un overlay del CDK—, así que el contenedor de avisos
  // desaparece del layout en vez de mudarse.
  imports: [RouterOutlet, SidebarComponent, HeaderComponent],
  changeDetection: ChangeDetectionStrategy.Eager,
  template: `
    <div class="layout-wrapper">
      <aside class="layout-sidebar">
        <app-sidebar />
      </aside>
      <div class="layout-main">
        <header class="layout-header">
          <app-header />
        </header>
        <main class="layout-content">
          <router-outlet />
        </main>
      </div>
    </div>
  `
})
export class MainLayoutComponent {}
