import Markdown from "markdown-to-jsx";
import { useMemo, type MouseEvent, type ReactNode } from "react";
import {
  resolverLink,
  rotuloTipo,
  type DocumentoAberto,
  type LigacaoVista,
} from "./conhecimento";
import { ConsultaReversa } from "./ConsultaReversa";
import { resolverImagem } from "./manual";
import { ImagemDaBase } from "./ManualAoVivo";
import { OPCOES_MARKDOWN_BASE } from "./markdownOpcoes";
import { separarNotasSeguidas } from "./notasDeRodape";

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
  aoEditar,
}: {
  documento: DocumentoAberto;
  aoAbrirDocumento: (caminho: string) => void;
  aoAbrirAnalise?: (projeto: string, id: string) => void;
  /** Mostra o botão "Editar". */
  aoEditar?: () => void;
}) {
  const campos = documento.campos.filter((c) => !NO_CABECALHO.has(c.chave));
  const saem = documento.ligacoes.filter((l) => l.sai);
  const chegam = documento.ligacoes.filter((l) => !l.sai);
  // Consulta reversa na tela da regra (#121).
  const regra = documento.campos.find((c) => c.chave === "id")?.valor;

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
        {aoEditar && (
          <button className="btn" type="button" onClick={aoEditar}>
            Editar
          </button>
        )}
        {documento.analise && aoAbrirAnalise && (
          <button
            className="btn"
            type="button"
            onClick={() => aoAbrirAnalise(documento.analise!.projeto, documento.analise!.id)}
          >
            Abrir na tela da análise
          </button>
        )}
        {documento.somenteLeitura && (
          <p className="muted" role="note">
            Documento externo, só leitura: cópia do repositório de docs
            {documento.externo && (
              <>
                {" "}
                (<span className="caminho">{documento.externo.caminho}</span> em{" "}
                {documento.externo.repositorio}, commit{" "}
                <code>{documento.externo.commit.slice(0, 7)}</code>,{" "}
                {documento.externo.confianca === "publicada"
                  ? "publicado pelo app"
                  : "escrito fora do app"}
                )
              </>
            )}
            . Para mudar, edite no repositório e sincronize. Não substitui a evidência do
            código.
          </p>
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

      {documento.tipo === "regra" && documento.projeto && regra && (
        <ConsultaReversa
          projeto={documento.projeto}
          regra={regra}
          aoAbrirDocumento={aoAbrirDocumento}
          aoAbrirAnalise={aoAbrirAnalise}
        />
      )}

      <CorpoMarkdown
        markdown={documento.markdown}
        caminho={documento.caminho}
        aoAbrirDocumento={aoAbrirDocumento}
      />
    </article>
  );
}

/**
 * O Markdown do documento renderizado, com os links para outros `.md` da base e as
 * notas de rodapé (todas, mesmo coladas: `[^1][^2]`) navegáveis. Sem
 * `aoAbrirDocumento` (a prévia do editor), os links para outros documentos aparecem
 * como texto.
 */
export function CorpoMarkdown({
  markdown,
  caminho,
  aoAbrirDocumento,
}: {
  markdown: string;
  /** Do documento, para resolver os links relativos. */
  caminho: string;
  aoAbrirDocumento?: (caminho: string) => void;
}) {
  const overrides = useMemo(() => {
    const Link = ({ href, children }: { href?: string; children?: ReactNode }) => {
      if (href?.startsWith("#")) return <a href={href}>{children}</a>;
      const destino = href ? resolverLink(caminho, href) : null;
      if (!destino || !aoAbrirDocumento) {
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
    // Prints do manual do usuário (#272): PNG da base lido pelo app.
    const Imagem = ({ src, alt }: { src?: string; alt?: string }) => {
      const naBase = src ? resolverImagem(caminho, src) : null;
      if (!naBase) {
        return (
          <span className="imagem" title={src}>
            {alt}
          </span>
        );
      }
      const [projeto, ...resto] = naBase.split("/");
      return <ImagemDaBase projeto={projeto} arquivo={resto.join("/")} alt={alt ?? ""} />;
    };
    return { a: { component: Link }, img: { component: Imagem } };
  }, [caminho, aoAbrirDocumento]);

  // A nota de rodapé leva à sua linha no fim do documento, sem mexer no endereço da janela.
  const irParaNota = (e: MouseEvent<HTMLDivElement>) => {
    const link = (e.target as HTMLElement).closest("a");
    const href = link?.getAttribute("href");
    if (!href?.startsWith("#") || href.length < 2) return;
    const alvo = Array.from(e.currentTarget.querySelectorAll("[id]")).find(
      (el) => el.id === href.slice(1),
    );
    e.preventDefault();
    alvo?.scrollIntoView?.({ block: "center" });
  };

  return (
    <div className="markdown" onClick={irParaNota}>
      <Markdown options={{ ...OPCOES_MARKDOWN_BASE, overrides }}>
        {separarNotasSeguidas(markdown)}
      </Markdown>
    </div>
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
