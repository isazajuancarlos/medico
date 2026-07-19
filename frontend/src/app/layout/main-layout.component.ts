import { Component } from '@angular/core';
import { RouterOutlet } from '@angular/router';
import { SidebarComponent } from './sidebar.component';
import { HeaderComponent } from './header.component';
import { ToastModule } from 'primeng/toast';

@Component({
  selector: 'app-main-layout',
  standalone: true,
  imports: [RouterOutlet, SidebarComponent, HeaderComponent, ToastModule],
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
    <p-toast />
  `
})
export class MainLayoutComponent {}
