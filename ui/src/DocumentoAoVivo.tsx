import type {
  Afirmacao,
  Confianca,
  EstadoAnalise,
  EstadoSessao,
  Evidencia,
  Pergunta,
  SessaoVista,
  VistaAnalise,
} from "./analise";

export const ROTULO_CONFIANCA: Record<Confianca, string> = {
  confirmado: "confirmado",
  inferido: "inferido",
  a_confirmar: "a confirmar",
};

export const ROTULO_SESSAO: Record<EstadoSessao, string> = {
  ativa: "Sessão ativa: o agente está trabalhando",
  "aguardando-analista": "Aguardando você: o Claude pede uma ação",
  concluida: "Resposta concluída: o agente espera a próxima mensagem",
  encerrada: "Sessão encerrada",
};

const ROTULO_ANALISE: Record<EstadoAnalise, string> = {
  criada: "criada",
  "em-andamento": "em andamento",
  concluida: "concluída, aguardando revisão",
  aprovada: "aprovada",
};

/**
 * Seções do documento "pergunta ao sistema" (`cba_docs::MODELOS`), na ordem do
 * template; seções de outros fluxos aparecem depois, com o nome do marcador.
 */
const SECOES_CONHECIDAS: [string, string][] = [
  ["resposta", "Resposta"],
  ["regras_encontradas", "Regras encontradas"],
  ["exemplo", "Exemplo"],
];
const TITULO_SECAO = Object.fromEntries(SECOES_CONHECIDAS) as Record<string, string>;
const ORDEM_SECAO = new Map(SECOES_CONHECIDAS.map(([s], i) => [s, i]));

const ESTADO_AFIRMACAO: Partial<Record<Afirmacao["estado"], string>> = {
  confirmada: "confirmada pelo analista",
  incorreta: "marcada como incorreta",
  aprofundar: "aprofundamento pedido",
  "a-revalidar": "a revalidar: o código mudou",
};

export function SeloSessao({ sessao }: { sessao: SessaoVista | null }) {
  if (!sessao) {
    return (
      <p className="selo-sessao sem-sessao" role="status" aria-label="Estado da sessão">
        Nenhuma sessão ligada ainda: ela aparece quando o agente chama a primeira ferramenta.
      </p>
    );
  }
  return (
    <p
      className={`selo-sessao sessao-${sessao.estado}`}
      role="status"
      aria-label="Estado da sessão"
      data-estado={sessao.estado}
    >
      {ROTULO_SESSAO[sessao.estado]}
    </p>
  );
}

export function chaveEvidencia(e: Evidencia): string {
  return `${e.arquivo}:${e.linhas}`;
}

interface Props {
  vista: VistaAnalise;
  /** Evidência aberta ao lado (destacada na lista). */
  evidenciaAberta?: string | null;
  aoAbrirEvidencia: (e: Evidencia) => void;
}

/** Documento da análise montado ao vivo a partir do estado gravado. */
export function DocumentoAoVivo({ vista, evidenciaAberta, aoAbrirEvidencia }: Props) {
  const { analise } = vista;
  const secoes = agrupar(analise.afirmacoes);
  const perguntas = [...analise.perguntas].sort(
    (a, b) => Number(a.estado !== "aberta") - Number(b.estado !== "aberta") || a.numero - b.numero,
  );
  const sugestoes = analise.sugestoes.filter((s) => s.estado !== "recusada");

  return (
    <article className="documento-ao-vivo" aria-label="Documento da análise">
      <header>
        <h2>
          {analise.id}: {analise.titulo}
        </h2>
        <p className="suave">
          Análise {ROTULO_ANALISE[analise.estado]} · versão {analise.versao}
        </p>
        <SeloSessao sessao={vista.sessao} />
      </header>

      <section aria-label="Pergunta">
        <h3>Pergunta</h3>
        <blockquote>{analise.entrada}</blockquote>
      </section>

      <section aria-label="Afirmações">
        <h3>Afirmações</h3>
        {secoes.length === 0 && <p className="suave">Nenhuma afirmação registrada ainda.</p>}
        {secoes.map(([secao, afirmacoes]) => (
          <div key={secao} className="secao">
            <h4>{TITULO_SECAO[secao] ?? secao}</h4>
            <ol className="afirmacoes">
              {afirmacoes.map((a) => (
                <li
                  key={a.numero}
                  className={`afirmacao estado-${a.estado}`}
                  data-testid={`afirmacao-${a.numero}`}
                >
                  <span className={`selo selo-${a.confianca}`}>
                    {ROTULO_CONFIANCA[a.confianca]}
                  </span>{" "}
                  <span className="texto">{a.texto}</span>
                  {ESTADO_AFIRMACAO[a.estado] && (
                    <span className="estado-afirmacao"> ({ESTADO_AFIRMACAO[a.estado]})</span>
                  )}
                  {a.estado === "incorreta" && (
                    <span className="comentario suave"> {a.comentario}</span>
                  )}
                  {a.evidencias.length > 0 && (
                    <ul className="evidencias" aria-label="Evidências">
                      {a.evidencias.map((e) => {
                        const chave = chaveEvidencia(e);
                        return (
                          <li key={chave}>
                            <button
                              type="button"
                              className="evidencia"
                              aria-pressed={evidenciaAberta === chave}
                              title={e.trecho}
                              onClick={() => aoAbrirEvidencia(e)}
                            >
                              {chave}
                            </button>
                            {e.estado === "a-revalidar" && (
                              <span className="suave"> a revalidar</span>
                            )}
                          </li>
                        );
                      })}
                    </ul>
                  )}
                </li>
              ))}
            </ol>
          </div>
        ))}
      </section>

      <section aria-label="Perguntas em aberto">
        <h3>Perguntas</h3>
        {perguntas.length === 0 ? (
          <p className="suave">Nenhuma pergunta registrada.</p>
        ) : (
          <ol className="perguntas">
            {perguntas.map((p) => (
              <li key={p.numero} className={`pergunta estado-${p.estado}`}>
                {p.texto}
                <EstadoDaPergunta pergunta={p} />
              </li>
            ))}
          </ol>
        )}
      </section>

      <section aria-label="Sugestões de alteração">
        <h3>Sugestões de alteração</h3>
        {sugestoes.length === 0 ? (
          <p className="suave">Nenhuma sugestão registrada.</p>
        ) : (
          <ol className="sugestoes">
            {sugestoes.map((s) => (
              <li key={s.numero}>
                <strong>{s.titulo || s.arquivo}</strong>
                {s.id && <span className="suave"> ({s.id})</span>}
                <p>{s.motivo}</p>
                {s.evidencia && (
                  <button
                    type="button"
                    className="evidencia"
                    aria-pressed={evidenciaAberta === chaveEvidencia(s.evidencia)}
                    onClick={() => s.evidencia && aoAbrirEvidencia(s.evidencia)}
                  >
                    {chaveEvidencia(s.evidencia)}
                  </button>
                )}
              </li>
            ))}
          </ol>
        )}
      </section>

      <p className="suave documento-caminho">
        Documento na base: <code>{vista.documento}</code>
      </p>
    </article>
  );
}

function EstadoDaPergunta({ pergunta }: { pergunta: Pergunta }) {
  if (pergunta.estado === "respondida")
    return <span className="suave"> Resposta: {pergunta.resposta}</span>;
  if (pergunta.estado === "descartada") return <span className="suave"> (descartada)</span>;
  return null;
}

/** Afirmações por seção, na ordem em que cada seção apareceu. */
function agrupar(afirmacoes: Afirmacao[]): [string, Afirmacao[]][] {
  const grupos = new Map<string, Afirmacao[]>();
  for (const a of [...afirmacoes].sort((x, y) => x.numero - y.numero)) {
    const lista = grupos.get(a.secao) ?? [];
    lista.push(a);
    grupos.set(a.secao, lista);
  }
  const ordem = (s: string) => ORDEM_SECAO.get(s) ?? ORDEM_SECAO.size;
  return [...grupos].sort(([a], [b]) => ordem(a) - ordem(b));
}
