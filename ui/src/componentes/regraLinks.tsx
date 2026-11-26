import type { MarkdownToJSX } from "markdown-to-jsx";
import type { ReactNode } from "react";

/** O host de um endereço (`https://github.com/o/r` → `github.com`) ou o próprio host. */
function hostDe(enderecoOuHost: string): string | null {
  try {
    return new URL(enderecoOuHost).host.toLowerCase();
  } catch {
    const host = enderecoOuHost.trim().toLowerCase();
    return host && !host.includes("/") ? host : null;
  }
}

/**
 * O link é do host permitido? `permitido` é um endereço (o da issue, por exemplo) ou
 * só o host. Endereço que não é URL absoluta nunca é do host.
 */
export function mesmoHost(link: string, permitido: string): boolean {
  const esperado = hostDe(permitido);
  if (!esperado) return false;
  try {
    return new URL(link).host.toLowerCase() === esperado;
  } catch {
    return false;
  }
}

/**
 * A regra única de links e imagens do texto rico (#398, #406):
 *
 * - âncora interna (`#nota`) fica como link e leva ao alvo dentro do texto;
 * - link do host permitido abre no navegador do sistema por `aoAbrir`;
 * - link de outro host (ou sem host permitido) aparece como texto, com o endereço
 *   no `title`;
 * - imagem não carrega: vira o link dela (pela mesma regra), com o texto alternativo.
 */
export function overridesDeLinks(
  hostPermitido: string | undefined,
  aoAbrir: ((link: string) => void) | undefined,
): MarkdownToJSX.Overrides {
  const Link = ({ href, children }: { href?: string; children?: ReactNode }) => {
    if (href?.startsWith("#")) return <a href={href}>{children}</a>;
    if (!href || !hostPermitido || !aoAbrir || !mesmoHost(href, hostPermitido)) {
      return (
        <span className="link-externo" title={href}>
          {children}
        </span>
      );
    }
    return (
      <a
        href={href}
        className="link"
        onClick={(e) => {
          e.preventDefault();
          aoAbrir(href);
        }}
      >
        {children}
      </a>
    );
  };
  const Imagem = ({ src, alt }: { src?: string; alt?: string }) => <Link href={src}>{alt || src}</Link>;
  return { a: { component: Link }, img: { component: Imagem } };
}
