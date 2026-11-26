// @ts-check
import { defineConfig } from 'astro/config';

// Teste no GitHub Pages: o site fica em https://deep-ion-ai.github.io/ctower-business-agents/.
// Com domínio próprio, `base` volta a ser '/' e `site` passa a ser o domínio.
export default defineConfig({
  site: 'https://deep-ion-ai.github.io',
  base: '/ctower-business-agents',
  output: 'static',
  devToolbar: { enabled: false },
  // pt-BR na raiz. /en/ fica reservado: adicionar 'en' aqui e um arquivo src/i18n/en.json.
  i18n: {
    defaultLocale: 'pt-br',
    locales: ['pt-br'],
  },
});
