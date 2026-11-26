// @ts-check
import { defineConfig } from 'astro/config';

// Domínio ainda não definido (pendência da #427): começa no *.pages.dev.
// Ajuste `site` quando o nome do projeto no Cloudflare Pages estiver definido.
export default defineConfig({
  site: 'https://ctower-business-agents.pages.dev',
  output: 'static',
  // pt-BR na raiz. /en/ fica reservado: adicionar 'en' aqui e um arquivo src/i18n/en.json.
  i18n: {
    defaultLocale: 'pt-br',
    locales: ['pt-br'],
  },
});
