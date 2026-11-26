import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, TituloSecao } from "./componentes";
import type {
  ApiPublicacao,
  MudancaDeDocumento,
  MudancaPrevista,
  PreviaPublicacao,
  Publicacao,
  TipoMudanca,
} from "./publicacao";

interface Props {
  api: ApiPublicacao;
  projeto: string;
  /** Abre o link do PR/MR no navegador. */
  abrirUrl: (url: string) => void | Promise<void>;
}

/** Quantos itens a prévia lista por tipo de mudança. */
const MAX_POR_TIPO = 20;

/** Folga entre o fim do painel e a borda de baixo da janela (inclui o padding do painel). */
const FOLGA_INFERIOR = 28;

/** Altura mínima do painel, mesmo em janela muito baixa. */
const ALTURA_MINIMA = 160;

const ROTULO_DOCUMENTOS: Record<TipoMudanca, string> = {
  novo: "Documentos novos",
  alterado: "Documentos alterados",
  removido: "Documentos removidos",
};

const ROTULO_PAGINA: Record<TipoMudanca, string> = {
  novo: "nova",
  alterado: "alterada",
  removido: "removida",
};

const RESULTADO: Record<Publicacao["publicado"]["situacao"], string> = {
  aberto: "Publicado: o PR/MR foi aberto.",
  atualizado: "Publicado: o PR/MR aberto recebeu as mudanças.",
  sem_mudancas: "Nada mudou: o repositório já tem esta versão da documentação.",
};

function ListaDocumentos({ tipo, itens }: { tipo: TipoMudanca; itens: MudancaDeDocumento[] }) {
  if (itens.length === 0) return null;
  return (
    <div className="publicacao-grupo">
      <TituloSecao nivel={4}>
        {ROTULO_DOCUMENTOS[tipo]} ({itens.length})
      </TituloSecao>
      <ul aria-label={ROTULO_DOCUMENTOS[tipo]}>
        {itens.slice(0, MAX_POR_TIPO).map((d) => (
          <li key={`${d.tipo}:${d.id}`}>
            <strong>{d.id}</strong>
            {d.titulo !== d.id && <> · {d.titulo}</>}
          </li>
        ))}
        {itens.length > MAX_POR_TIPO && <li className="muted">e mais {itens.length - MAX_POR_TIPO}</li>}
      </ul>
    </div>
  );
}

function PaginasAfetadas({ itens }: { itens: MudancaPrevista[] }) {
  if (itens.length === 0) return null;
  return (
    <details className="publicacao-grupo">
      <summary>Páginas afetadas ({itens.length})</summary>
      <ul aria-label="Páginas afetadas">
        {itens.slice(0, MAX_POR_TIPO * 2).map((m) => (
          <li key={m.caminho}>
            <code>{m.caminho}</code> <span className="muted">({ROTULO_PAGINA[m.tipo]})</span>
          </li>
        ))}
        {itens.length > MAX_POR_TIPO * 2 && (
          <li className="muted">e mais {itens.length - MAX_POR_TIPO * 2}</li>
        )}
      </ul>
    </details>
  );
}

/** Por que não há o que publicar. */
function motivoSemPublicacao(p: PreviaPublicacao): string {
  if (p.branch_atualizada) {
    return `O PR/MR da branch ${p.branch} já tem esta versão: não há nada a enviar.`;
  }
  if (p.aprovados === 0) return "Ainda não há documentos aprovados neste projeto.";
  return "Nada a publicar: o repositório já tem esta versão da documentação aprovada.";
}

/**
 * Altura máxima do painel: até a borda de baixo da janela, a partir de onde ele
 * começa. Assim, numa janela baixa, o corpo rola e as ações (aviso de segredos e
 * botão "Publicar") ficam sempre visíveis no rodapé.
 */
function useAlturaAteOFim<T extends HTMLElement>() {
  const ref = useRef<T>(null);
  const [altura, setAltura] = useState<number | undefined>(undefined);
  useLayoutEffect(() => {
    const medir = () => {
      const el = ref.current;
      if (!el) return;
      const topo = el.getBoundingClientRect().top;
      setAltura(Math.max(ALTURA_MINIMA, Math.floor(window.innerHeight - topo - FOLGA_INFERIOR)));
    };
    medir();
    window.addEventListener("resize", medir);
    return () => window.removeEventListener("resize", medir);
  }, []);
  return { ref, altura };
}

/**
 * Publicação da documentação aprovada no destino da documentação do projeto
 * (#276, #289): as páginas legíveis vão numa branch própria com PR/MR. A prévia
 * diz o que muda por documento ("2 regras novas, 1 alterada"), as páginas
 * afetadas e se o PR/MR aberto já tem esta versão; pede a conferência de segredos
 * e dados pessoais e devolve o link. O corpo rola; as ações ficam no rodapé.
 */
export function PublicacaoDocs({ api, projeto, abrirUrl }: Props) {
  const [falhaLink, setFalhaLink] = useState<string | null>(null);
  const [previa, setPrevia] = useState<PreviaPublicacao | null>(null);
  const [carregando, setCarregando] = useState(true);
  const [conferido, setConferido] = useState(false);
  const [publicando, setPublicando] = useState(false);
  const [resultado, setResultado] = useState<Publicacao | null>(null);
  const [erro, setErro] = useState<string | null>(null);
  const { ref, altura } = useAlturaAteOFim<HTMLElement>();

  const carregar = useCallback(async () => {
    setErro(null);
    setResultado(null);
    setConferido(false);
    setCarregando(true);
    try {
      setPrevia(await api.previa(projeto));
    } catch (e) {
      setPrevia(null);
      setErro(mensagemDe(e));
    } finally {
      setCarregando(false);
    }
  }, [api, projeto]);

  useEffect(() => {
    void carregar();
  }, [carregar]);

  const publicar = async () => {
    setErro(null);
    setPublicando(true);
    try {
      const r = await api.publicar(projeto);
      setResultado(r);
      setPrevia(r.previa);
    } catch (e) {
      setErro(mensagemDe(e));
    } finally {
      setPublicando(false);
    }
  };

  const docs = previa?.documentos;
  const doTipo = (tipo: TipoMudanca) => docs?.documentos.filter((d) => d.mudanca === tipo) ?? [];
  const link = resultado?.publicado.link;
  const podePublicar = previa !== null && !carregando && !previa.nada_a_publicar && !resultado;

  return (
    <section
      ref={ref}
      className="publicacao"
      aria-label="Publicação da documentação"
      style={altura === undefined ? undefined : { maxHeight: `${altura}px` }}
    >
      <TituloSecao nivel={3}>Publicar a documentação</TituloSecao>
      <div className="publicacao-corpo" data-testid="publicacao-corpo">
        <p className="muted">
          A documentação aprovada do projeto vira páginas legíveis (índice, funcionalidades, regras,
          glossário e análises) e vai para o repositório numa branch própria, com um PR/MR para
          revisão. Nada é gravado direto na branch base.
        </p>
        {carregando && <p role="status">Conferindo o que muda no repositório…</p>}
        {previa && !carregando && (
          <>
            <dl className="publicacao-destino">
              <dt>Destino</dt>
              <dd>{previa.destino}</dd>
              <dt>Branch</dt>
              <dd>
                <code>{previa.branch}</code> → <code>{previa.base}</code>
              </dd>
              <dt>Aprovados</dt>
              <dd>
                {previa.aprovados} documentos
                {previa.paginas > 0 && <> em {previa.paginas} páginas</>}
              </dd>
            </dl>
            {previa.nada_a_publicar ? (
              <p role="status">{motivoSemPublicacao(previa)}</p>
            ) : (
              <>
                {docs && docs.texto ? (
                  <p className="publicacao-resumo">
                    <strong>O que muda:</strong> {docs.texto}.
                  </p>
                ) : (
                  previa.mudancas.length === 0 && <p>Nada muda na branch base.</p>
                )}
                {previa.formato_antigo && (
                  <p className="muted">
                    A publicação anterior está no formato antigo (os arquivos da base): eles saem e as
                    páginas legíveis entram no lugar. Arquivos escritos à mão na pasta ficam.
                  </p>
                )}
                <div aria-label="O que muda">
                  <ListaDocumentos tipo="novo" itens={doTipo("novo")} />
                  <ListaDocumentos tipo="alterado" itens={doTipo("alterado")} />
                  <ListaDocumentos tipo="removido" itens={doTipo("removido")} />
                  <PaginasAfetadas itens={previa.mudancas} />
                </div>
                {previa.pendente_na_branch && (
                  <p className="muted">
                    A branch <code>{previa.branch}</code> já tem uma publicação ainda não mesclada: ela
                    é atualizada com esta versão.
                  </p>
                )}
              </>
            )}
            {previa.avisos.length > 0 && (
              <div className="publicacao-grupo">
                <TituloSecao nivel={4}>Ficaram de fora ({previa.avisos.length})</TituloSecao>
                <ul aria-label="Ficaram de fora">
                  {previa.avisos.map((a) => (
                    <li key={a}>{a}</li>
                  ))}
                </ul>
              </div>
            )}
          </>
        )}
      </div>
      <div className="publicacao-acoes" data-testid="publicacao-acoes">
        {podePublicar && (
          <>
            <Alerta tom="atencao" role="note" titulo="As páginas mostram os documentos como estão na base.">
              Antes de publicar, confira se não há segredos (senhas, tokens, chaves, endereços
              internos) nem dados pessoais (nomes, CPF, e-mails, telefones) que não podem ir para o
              repositório.
              <label className="publicacao-conferido">
                <input type="checkbox" checked={conferido} onChange={(e) => setConferido(e.target.checked)} />{" "}
                Conferi: não há segredos nem dados pessoais nos documentos
              </label>
            </Alerta>
            <button
              className="btn btn-primary"
              type="button"
              disabled={!conferido || publicando}
              onClick={publicar}
            >
              {publicando ? "Publicando…" : "Publicar"}
            </button>
          </>
        )}
        {resultado && (
          <div role="status">
            <p>{RESULTADO[resultado.publicado.situacao]}</p>
            {link && (
              <p>
                <a
                  href={link}
                  onClick={(e) => {
                    e.preventDefault();
                    setFalhaLink(null);
                    Promise.resolve(abrirUrl(link)).catch((err: unknown) =>
                      setFalhaLink(`Não foi possível abrir o link. ${mensagemDe(err)}`),
                    );
                  }}
                >
                  Abrir o PR/MR
                </a>{" "}
                <span className="muted">{link}</span>
              </p>
            )}
            {falhaLink && <Alerta tom="erro" titulo={falhaLink} />}
          </div>
        )}
        {erro && (
          <>
            <Alerta
              tom="erro"
              titulo={erro}
              acoes={
                <button className="btn" type="button" disabled={carregando} onClick={carregar}>
                  Tentar de novo
                </button>
              }
            />
          </>
        )}
      </div>
    </section>
  );
}
