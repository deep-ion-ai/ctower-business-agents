import { useMemo, type ReactNode } from "react";
import { resolverLink, rotuloTipo, type DocumentoAberto, type LigacaoVista } from "./conhecimento";
import { TextoRico } from "./componentes";
import { ConsultaReversa } from "./ConsultaReversa";
import { ErroTecnico } from "./erroTecnico";
import { resolverImagem } from "./manual";
import { ImagemDaBase } from "./ManualAoVivo";

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
 * Renderizador: o {@link TextoRico} (`markdown-to-jsx` 7, MIT, sem dependências). Ele
 * monta elementos React (nada de `innerHTML`), com `disableParsingRawHTML` o HTML do
 * arquivo vira texto e o sanitizador embutido descarta URLs `javascript:`/`data:`. O
 * `react-markdown` faria o mesmo com dezenas de pacotes do ecosprojeto unified.
 */
export function DocumentoBase({
  documento,
  aoAbrirDocumento,
  aoAbrirAnalise,
  aoEditar,
  aoAbrirEmAba,
  nomeDaArea,
}: {
  documento: DocumentoAberto;
  aoAbrirDocumento: (caminho: string) => void;
  aoAbrirAnalise?: (projeto: string, id: string) => void;
  /** Mostra o botão "Editar". */
  aoEditar?: () => void;
  /** Mostra "Abrir em aba": a página inteira do documento. */
  aoAbrirEmAba?: () => void;
  /** Nome legível da área (o do `projeto.md`); sem ele, o identificador. */
  nomeDaArea?: (id: string) => string;
}) {
  const campos = documento.campos.filter((c) => !NO_CABECALHO.has(c.chave));
  const saem = documento.ligacoes.filter((l) => l.sai);
  const chegam = documento.ligacoes.filter((l) => !l.sai);
  // Consulta reversa na tela da regra (#121).
  const regra = documento.campos.find((c) => c.chave === "id")?.valor;
  const ehRegra = documento.tipo === "regra" && !!documento.projeto && !!regra;
  const area = documento.campos.find((c) => c.chave === "area")?.valor;

  return (
    <article className="documento-base" aria-label={documento.titulo}>
      <header>
        <p className="muted">
          {rotuloTipo(documento.tipo)}
          {regra && documento.tipo === "regra" && ` ${regra}`}
          {documento.projeto && ` · ${documento.projeto}`}
          {area && ` · ${nomeDaArea ? nomeDaArea(area) : area}`}
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
        <div className="acoes">
          {aoEditar && (
            <button className="btn" type="button" onClick={aoEditar}>
              Editar
            </button>
          )}
          {aoAbrirEmAba && (
            <button className="btn" type="button" onClick={aoAbrirEmAba}>
              Abrir em aba
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
        </div>
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
            . Para mudar, edite no repositório e sincronize. Não substitui a evidência do código.
          </p>
        )}
        {documento.problema && (
          <p className="error-text" role="alert">
            Este documento tem um problema e não pôde ser lido por inteiro:{" "}
            <ErroTecnico mensagem={documento.problema} />
          </p>
        )}
      </header>

      {/* O texto vem primeiro (B-05): o resto é apoio. */}
      <CorpoMarkdown
        markdown={documento.markdown}
        caminho={documento.caminho}
        aoAbrirDocumento={aoAbrirDocumento}
      />

      {ehRegra && (
        <ConsultaReversa
          projeto={documento.projeto!}
          regra={regra!}
          aoAbrirDocumento={aoAbrirDocumento}
          aoAbrirAnalise={aoAbrirAnalise}
          comCodigo
        />
      )}

      {(saem.length > 0 || chegam.length > 0) && (
        <nav className="ligacoes" aria-label="Ligações">
          {saem.length > 0 && <Ligacoes titulo="Cita" ligacoes={saem} aoAbrir={aoAbrirDocumento} />}
          {chegam.length > 0 && (
            <Ligacoes titulo="Citado por" ligacoes={chegam} aoAbrir={aoAbrirDocumento} />
          )}
        </nav>
      )}

      <details className="detalhes-arquivo">
        <summary>Detalhes do arquivo</summary>
        <dl className="campos-documento">
          <div>
            <dt>arquivo</dt>
            <dd>{documento.caminho}</dd>
          </div>
          {campos.map((c) => (
            <div key={c.chave}>
              <dt>{c.chave}</dt>
              <dd>{c.valor}</dd>
            </div>
          ))}
        </dl>
      </details>
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

  // As notas de rodapé (inclusive as coladas) e a ida até elas ficam no TextoRico.
  return <TextoRico texto={markdown} overrides={overrides} />;
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
