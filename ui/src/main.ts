import './app.css';
import { mount } from 'svelte';
import App from './App.svelte';

// Optional theme override (data-theme); without it the OS preference decides.
try {
  const th = localStorage.getItem('mango-theme');
  if (th === 'light' || th === 'dark') document.documentElement.dataset.theme = th;
} catch { /* storage unavailable */ }

mount(App, { target: document.getElementById('app')! });
