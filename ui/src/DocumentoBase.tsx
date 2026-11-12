import Markdown from "markdown-to-jsx";
import type { ReactNode } from "react";
import {
  resolverLink,
  rotuloTipo,
  type DocumentoAberto,
  type LigacaoVista,
} from "./conhecimento";

/** Campos do frontmatter que já aparecem no cabeçalho. */
const NO_CABECALHO = new Set(["tipo", "titulo", "termo", "status"]);

const ROTULO_CAMPO: Record<string, string> = {
  origem: "Origem",
  analise: "Análise",
};

/**
 * Documento da base renderizado: o Markdown (sem HTML cru: tags no texto aparecem
 * como texto), os campos do frontmatter e as ligações com outros documentos. Os
 * links para outros `.md` da base abrem o documento no app; os demais ficam como
 * texto, porque o app não abre endereços externos daqui.
 *
 * Renderizador: `markdown-to-jsx` 7 (MIT, sem dependências). Ele monta elementos
 * React (nada de `innerHTML`), com `disableParsingRawHTML` o HTML do arquivo vira
 * texto e o sanitizador embutido descarta URLs `javascript:`/`data:`. O
 * `react-markdown` faria o mesmo com dezenas de pacotes do ecosprojeto unified.
 */
export function DocumentoBase({
  documento,
  aoAbrirDocumento,
  aoAbrirAnalise,
}: {
  documento: DocumentoAberto;
  aoAbrirDocumento: (caminho: string) => void;
  aoAbrirAnalise?: (projeto: string, id: string) => void;
}) {
  const campos = documento.campos.filter((c) => !NO_CABECALHO.has(c.chave));
  const saem = documento.ligacoes.filter((l) => l.sai);
  const chegam = documento.ligacoes.filter((l) => !l.sai);

  const Link = ({ href, children }: { href?: string; children?: ReactNode }) => {
    if (href?.startsWith("#")) return <a href={href}>{children}</a>;
    const destino = href ? resolverLink(documento.caminho, href) : null;
    if (!destino) {
      return (
        <span className="link-externo" title={href}>
          {children}
        </span>
      );
    }
    return (
      <a
        href={`#${destino}`}
        className="link"
        onClick={(e) => {
          e.preventDefault();
          aoAbrirDocumento(destino);
        }}
      >
        {children}
      </a>
    );
  };

  return (
    <article className="documento-base" aria-label={documento.titulo}>
      <header>
        <p className="muted">
          {rotuloTipo(documento.tipo)}
          {documento.projeto && ` · ${documento.projeto}`} · <code>{documento.caminho}</code>
        </p>
        <h2>
          {documento.titulo}{" "}
          {documento.status && (
            <span
              className="badge"
              data-tone={documento.status === "a-revalidar" ? "attention" : undefined}
            >
              {documento.status === "a-revalidar" ? "a revalidar" : documento.status}
            </span>
          )}
        </h2>
        {documento.analise && aoAbrirAnalise && (
          <button
            className="btn"
            type="button"
            onClick={() => aoAbrirAnalise(documento.analise!.projeto, documento.analise!.id)}
          >
            Abrir na tela da análise
          </button>
        )}
        {documento.problema && (
          <p className="error-text" role="alert">
            Frontmatter ilegível: {documento.problema}
          </p>
        )}
      </header>

      {campos.length > 0 && (
        <dl className="campos-documento">
          {campos.map((c) => (
            <div key={c.chave}>
              <dt>{c.chave}</dt>
              <dd>{c.valor}</dd>
            </div>
          ))}
        </dl>
      )}

      {(saem.length > 0 || chegam.length > 0) && (
        <nav className="ligacoes" aria-label="Ligações">
          {saem.length > 0 && (
            <Ligacoes titulo="Cita" ligacoes={saem} aoAbrir={aoAbrirDocumento} />
          )}
          {chegam.length > 0 && (
            <Ligacoes titulo="Citado por" ligacoes={chegam} aoAbrir={aoAbrirDocumento} />
          )}
        </nav>
      )}

      <div className="markdown">
        <Markdown
          options={{
            disableParsingRawHTML: true,
            forceBlock: true,
            overrides: { a: { component: Link } },
          }}
        >
          {documento.markdown}
        </Markdown>
      </div>
    </article>
  );
}

function Ligacoes({
  titulo,
  ligacoes,
  aoAbrir,
}: {
  titulo: string;
  ligacoes: LigacaoVista[];
  aoAbrir: (caminho: string) => void;
}) {
  return (
    <div>
      <h3>{titulo}</h3>
      <ul>
        {ligacoes.map((l) => (
          <li key={`${l.campo}:${l.caminho}`}>
            <span className="muted">{ROTULO_CAMPO[l.campo] ?? l.campo}: </span>
            {l.existe ? (
              <button type="button" className="link" onClick={() => aoAbrir(l.caminho)}>
                {l.titulo ?? l.caminho}
              </button>
            ) : (
              <span title="O documento não está na base">
                {l.caminho} <span className="muted">(não encontrado)</span>
              </span>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
