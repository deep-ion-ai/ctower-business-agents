import type { APIRoute } from 'astro';

// Os buscadores só leem o robots.txt na raiz do domínio. No GitHub Pages do projeto
// (deep-ion-ai.github.io/ctower-business-agents/) ele vale de fato com domínio próprio;
// até lá, o sitemap também é anunciado pela tag <link rel="sitemap"> da página.
export const GET: APIRoute = ({ site }) => {
  const base = import.meta.env.BASE_URL.replace(/\/?$/, '/');
  const sitemap = new URL(`${base}sitemap.xml`, site).href;
  const body = `User-agent: *
Allow: /

Sitemap: ${sitemap}
`;
  return new Response(body, { headers: { 'Content-Type': 'text/plain; charset=utf-8' } });
};
