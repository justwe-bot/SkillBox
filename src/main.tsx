import React from 'react'
import ReactDOM from 'react-dom/client'
import { HashRouter } from 'react-router-dom'
import App from './App'
import { LanguageProvider } from './lib/i18n-context'
import './styles/main.css'

ReactDOM.createRoot(document.getElementById('app') as HTMLElement).render(
  <React.StrictMode>
    <LanguageProvider>
      <HashRouter>
        <App />
      </HashRouter>
    </LanguageProvider>
  </React.StrictMode>,
)
