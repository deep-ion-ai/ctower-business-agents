// @ts-check
import { defineConfig } from 'astro/config';

// Teste no GitHub Pages: o site fica em https://deep-ion-ai.github.io/ctower-business-agents/.
// Com domínio próprio, `base` volta a ser '/' e `site` passa a ser o domínio.
export default defineConfig({
  site: 'https://deep-ion-ai.github.io',
  base: '/ctower-business-agents',
  output: 'static',
  devToolbar: { enabled: false },
  // CSP pela tag <meta> (o GitHub Pages não deixa definir cabeçalhos HTTP). O Astro
  // calcula os hashes dos scripts e estilos inline do build.
  security: {
    csp: {
      algorithm: 'SHA-256',
      directives: [
        "default-src 'self'",
        "img-src 'self' data:",
        "font-src 'self' https://fonts.gstatic.com",
        "connect-src 'self'",
        "object-src 'none'",
        "base-uri 'self'",
        "form-action 'none'",
        'upgrade-insecure-requests',
      ],
      styleDirective: { resources: ["'self'", 'https://fonts.googleapis.com'] },
      scriptDirective: { resources: ["'self'"] },
    },
  },
  // pt-BR na raiz. /en/ fica reservado: adicionar 'en' aqui e um arquivo src/i18n/en.json.
  i18n: {
    defaultLocale: 'pt-br',
    locales: ['pt-br'],
  },
});
