import { useCallback, useEffect, useState } from "react";
import { mensagemDe } from "./analise";
import { Alerta, BarraAcoes, Cartao, EstadoVazio, Secao, Selo, TituloSecao } from "./componentes";
import { contar } from "./formatacao";
import {
  mudou,
  ROTULO_CLASSE,
  type ApiMudancas,
  type PropostaDeRegra,
  type RegraAfetada,
  type RelatorioDeMudancas,
} from "./mudancas";
import type { TelaTerminal } from "./telaXterm";
import { TerminalClaude } from "./TerminalClaude";
import type { ApiTerminal } from "./terminal";
import "./mudancas.css";

type Estado =
  | { tipo: "carregando" }
  | { tipo: "pronto"; relatorio: RelatorioDeMudancas }
  | { tipo: "falha"; mensagem: string };

const vazio = (t: string) => (t.trim() ? t.trim() : null);

/**
 * "O que mudou no código" (#124), aberto pela tela do projeto: o relatório entre
 * dois pontos do repositório (padrão: desde a última análise aprovada até a cópia
 * de trabalho), a nota de mudanças em Markdown, as propostas do agente para
 * confirmar ou descartar e a sessão "explicar mudanças" com o Claude.
 */
export function MudancasCodigo({
  api,
  projeto,
  aoFechar,
  apiTerminal,
  criarTela,
}: {
  api: ApiMudancas;
  projeto: string;
  aoFechar: () => void;
  /** Para testes. */
  apiTerminal?: ApiTerminal;
  /** Para testes. */
  criarTela?: () => TelaTerminal;
}) {
  const [de, setDe] = useState("");
  const [ate, setAte] = useState("");
  // O intervalo do relatório mostrado (o dos campos só vale ao pedir de novo).
  const [intervalo, setIntervalo] = useState<{ de: string | null; ate: string | null }>({
    de: null,
    ate: null,
  });
  const [estado, setEstado] = useState<Estado>({ tipo: "carregando" });
  const [propostas, setPropostas] = useState<PropostaDeRegra[]>([]);
  const [aviso, setAviso] = useState<string | null>(null);
  const [pedido, setPedido] = useState<string | null>(null);

  const carregarPropostas = useCallback(() => {
    api
      .propostas(projeto)
      .then(setPropostas)
      .catch((e: unknown) => setAviso(mensagemDe(e)));
  }, [api, projeto]);

  useEffect(() => {
    let ativo = true;
    setEstado({ tipo: "carregando" });
    api
      .relatorio(projeto, intervalo.de, intervalo.ate)
      .then((relatorio) => ativo && setEstado({ tipo: "pronto", relatorio }))
      .catch((e: unknown) => ativo && setEstado({ tipo: "falha", mensagem: mensagemDe(e) }));
    carregarPropostas();
    return () => {
      ativo = false;
    };
  }, [api, projeto, intervalo, carregarPropostas]);

  const exportar = () => {
    setAviso(null);
    api
      .escolherDestino()
      .then((destino) =>
        destino ? api.exportarNota(projeto, intervalo.de, intervalo.ate, destino) : null,
      )
      .then((gravado) => gravado && setAviso(`Nota de mudanças gravada em ${gravado}.`))
      .catch((e: unknown) => setAviso(mensagemDe(e)));
  };

  const explicar = () => {
    setAviso(null);
    api
      .pedido(intervalo.de, intervalo.ate)
      .then(setPedido)
      .catch((e: unknown) => setAviso(mensagemDe(e)));
  };

  const pendentes = propostas.filter((p) => p.situacao === "pendente");

  return (
    <Secao
      titulo="O que mudou no código"
      nivel={3}
      acoes={
        <button type="button" className="btn" onClick={aoFechar}>
          Fechar
        </button>
      }
    >

      <form
        className="mudancas-intervalo"
        aria-label="Intervalo"
        onSubmit={(e) => {
          e.preventDefault();
          setIntervalo({ de: vazio(de), ate: vazio(ate) });
        }}
      >
        <label>
          De (commit, tag ou branch)
          <input
            value={de}
            placeholder="última análise aprovada"
            onChange={(e) => setDe(e.target.value)}
          />
        </label>
        <label>
          Até
          <input value={ate} placeholder="cópia de trabalho" onChange={(e) => setAte(e.target.value)} />
        </label>
        <button type="submit" className="btn">
          Comparar
        </button>
      </form>

      {aviso && (
        <p className="muted" role="status">
          {aviso}
        </p>
      )}
      {estado.tipo === "carregando" && <p className="muted">Comparando as versões do código…</p>}
      {estado.tipo === "falha" && <Alerta tom="erro" titulo={estado.mensagem} />}
      {estado.tipo === "pronto" && (
        <Relatorio
          relatorio={estado.relatorio}
          aoExportar={exportar}
          aoExplicar={pedido === null ? explicar : undefined}
        />
      )}

      <Propostas
        api={api}
        projeto={projeto}
        pendentes={pendentes}
        aoMudar={(texto) => {
          setAviso(texto);
          carregarPropostas();
        }}
      />

      {pedido !== null && (
        <section aria-label="Explicar mudanças com o Claude">
          <TituloSecao
            nivel={4}
            acoes={
              <button
                type="button"
                className="btn"
                onClick={() => {
                  setPedido(null);
                  carregarPropostas();
                }}
              >
                Encerrar a sessão
              </button>
            }
          >
            Explicar mudanças com o Claude
          </TituloSecao>
          <p className="muted">
            O agente registra uma proposta por regra; confirme ou descarte cada uma acima.
          </p>
          <div className="mudancas-terminal">
            <TerminalClaude
              projeto={projeto}
              pedido={{ promptInicial: pedido }}
              api={apiTerminal}
              criarTela={criarTela}
            />
          </div>
        </section>
      )}
    </Secao>
  );
}

function descreverPonto(p: RelatorioDeMudancas["de"]): string {
  const commit = p.commit ? ` (${p.commit.slice(0, 8)})` : "";
  if (p.analise) return `última análise aprovada, ${p.analise.id} · ${p.analise.titulo}${commit}`;
  return `${p.referencia}${commit}`;
}

function Relatorio({
  relatorio: r,
  aoExportar,
  aoExplicar,
}: {
  relatorio: RelatorioDeMudancas;
  aoExportar: () => void;
  aoExplicar?: () => void;
}) {
  const mudaram = r.regras.filter((x) => mudou(x.classe)).length;
  return (
    <>
      <p>
        Desde <strong>{descreverPonto(r.de)}</strong> até <strong>{descreverPonto(r.ate)}</strong>:{" "}
        {contar(r.arquivos_alterados, "arquivo alterado", "arquivos alterados")},{" "}
        {contar(r.com_evidencias.length, "citado pela base", "citados pela base")},{" "}
        {contar(mudaram, "regra com o trecho alterado", "regras com o trecho alterado")}.
      </p>
      <BarraAcoes
        principal={
          aoExplicar && r.regras.length > 0
            ? { texto: "Explicar mudanças com o Claude", aoEscolher: aoExplicar }
            : undefined
        }
        secundarias={[{ texto: "Exportar a nota de mudanças", aoEscolher: aoExportar }]}
        rotulo="Ações do relatório"
      />
      {r.problemas.length > 0 && (
        <ul className="muted" aria-label="Problemas">
          {r.problemas.map((p) => (
            <li key={p}>{p}</li>
          ))}
        </ul>
      )}

      <section aria-label="Regras afetadas">
        <TituloSecao nivel={4}>Regras afetadas</TituloSecao>
        {r.regras.length === 0 && (
          <EstadoVazio>Nenhuma regra documentada cita os arquivos alterados.</EstadoVazio>
        )}
        {r.regras.map((regra) => (
          <Regra key={regra.id} regra={regra} />
        ))}
      </section>

      <section aria-label="Arquivos alterados citados pela base">
        <TituloSecao nivel={4}>Arquivos alterados citados pela base</TituloSecao>
        {r.com_evidencias.length === 0 && <EstadoVazio>Nenhum.</EstadoVazio>}
        <ul>
          {r.com_evidencias.map((a) => (
            <li key={a.arquivo}>
              <code>{a.arquivo}</code> ({a.situacao}):{" "}
              {[
                ...a.regras,
                ...a.termos.map((t) => `termo ${t}`),
                ...a.funcionalidades,
                ...a.analises.map((x) => `análise ${x}`),
              ].join(", ")}
            </li>
          ))}
        </ul>
      </section>

      <section aria-label="Arquivos sem evidência">
        <TituloSecao nivel={4}>Arquivos alterados sem evidência na base</TituloSecao>
        {r.sem_evidencias.length === 0 ? (
          <EstadoVazio>Nenhum.</EstadoVazio>
        ) : (
          <p className="muted">Possíveis regras novas, ainda não documentadas.</p>
        )}
        {r.sem_evidencias.map((g) => (
          <div key={`${g.area ?? "pasta"}:${g.nome}`}>
            <strong>{g.area ? `Área ${g.nome}` : `Pasta ${g.nome}`}</strong>
            <ul>
              {g.arquivos.map((a) => (
                <li key={a.arquivo}>
                  <code>{a.arquivo}</code> ({a.situacao})
                </li>
              ))}
            </ul>
          </div>
        ))}
      </section>
    </>
  );
}

function Regra({ regra }: { regra: RegraAfetada }) {
  return (
    <article className="mudancas-regra" aria-label={`${regra.id} · ${regra.titulo}`}>
      <div>
        <strong>
          {regra.id} · {regra.titulo}
        </strong>{" "}
        <Selo tom={mudou(regra.classe) ? "atencao" : "neutro"}>{ROTULO_CLASSE[regra.classe]}</Selo>
      </div>
      {regra.trechos.map((t) => (
        <div key={`${t.arquivo}:${t.linhas}`}>
          <span className="muted">
            <code>{t.arquivo}</code>, linhas {t.linhas}
            {t.linhas_novas && t.linhas_novas !== t.linhas && ` (agora ${t.linhas_novas})`}:{" "}
            {ROTULO_CLASSE[t.classe]}
          </span>
          {t.diff && <pre className="diff">{t.diff}</pre>}
        </div>
      ))}
    </article>
  );
}

function Propostas({
  api,
  projeto,
  pendentes,
  aoMudar,
}: {
  api: ApiMudancas;
  projeto: string;
  pendentes: PropostaDeRegra[];
  aoMudar: (aviso: string) => void;
}) {
  return (
    <section aria-label="Propostas do agente">
      <TituloSecao nivel={4}>Propostas do agente</TituloSecao>
      {pendentes.length === 0 && <EstadoVazio>Nenhuma proposta pendente.</EstadoVazio>}
      {pendentes.map((p) => (
        <Proposta key={p.numero} api={api} projeto={projeto} proposta={p} aoMudar={aoMudar} />
      ))}
    </section>
  );
}

function Proposta({
  api,
  projeto,
  proposta: p,
  aoMudar,
}: {
  api: ApiMudancas;
  projeto: string;
  proposta: PropostaDeRegra;
  aoMudar: (aviso: string) => void;
}) {
  const [texto, setTexto] = useState(p.enunciado ?? "");
  const [ocupado, setOcupado] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const agir = (acao: () => Promise<string>) => {
    setOcupado(true);
    setErro(null);
    acao()
      .then(aoMudar)
      .catch((e: unknown) => setErro(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };
  return (
    <Cartao rotulo={`Proposta para ${p.regra}`} className="mudancas-proposta">
      <p>
        <strong>{p.regra}</strong>:{" "}
        {p.comportamento_mudou ? "o comportamento mudou" : "o comportamento não mudou"}.{" "}
        {p.explicacao}
      </p>
      <label>
        Texto da regra (deixe vazio para manter o atual)
        <textarea rows={3} value={texto} onChange={(e) => setTexto(e.target.value)} />
      </label>
      {erro && <Alerta tom="erro" titulo={erro} />}
      <BarraAcoes
        principal={{
          texto: "Confirmar",
          desabilitado: ocupado,
          aoEscolher: () =>
            agir(() =>
              api
                .confirmar(projeto, p.numero, null, vazio(texto))
                .then(
                  (r) =>
                    `${r.id} atualizada; a versão anterior ficou no histórico (${contar(r.versoes_anteriores, "versão", "versões")}).`,
                ),
            ),
        }}
        secundarias={[
          {
            texto: "Descartar",
            desabilitado: ocupado,
            aoEscolher: () =>
              agir(() => api.descartar(projeto, p.numero).then(() => `Proposta para ${p.regra} descartada.`)),
          },
        ]}
        rotulo={`Ações da proposta para ${p.regra}`}
      />
    </Cartao>
  );
}
