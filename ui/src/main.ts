import './app.css';
import { mount } from 'svelte';
import App from './App.svelte';
import { installInputModality } from './lib/inputModality';

// Optional theme override (data-theme); without it the OS preference decides.
try {
  const th = localStorage.getItem('mango-theme');
  if (th === 'light' || th === 'dark') document.documentElement.dataset.theme = th;
} catch { /* storage unavailable */ }

// focus rings only while the keyboard is really used (never after a mouse click)
installInputModality();

mount(App, { target: document.getElementById('app')! });

// Dev only: lets the browser console and the visual checks jump to any state (tree-shaken from production builds).
if (import.meta.env.DEV) {
  void import('./lib/stores.svelte').then((m) => { (window as unknown as { __dev: unknown }).__dev = m; });
}
