import React from 'react';
import ReactDOM from 'react-dom/client';
import './tokens.css';
import App from './App';

// Suppress the native right-click menu (WebKitGTK/WebView2). Editable fields
// keep it so paste still works there.
document.addEventListener('contextmenu', (event) => {
  const target = event.target as HTMLElement | null;
  if (target?.closest('input, textarea, [contenteditable="true"]')) return;
  event.preventDefault();
});

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
