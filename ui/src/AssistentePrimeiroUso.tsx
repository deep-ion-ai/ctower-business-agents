import { useCallback, useState } from "react";
import { mensagemDe } from "./analise";
import { CadastroSistema, ResumoDoInventario } from "./CadastroSistema";
import { EscolhaBase } from "./EscolhaBase";
import { EscolhaCliente } from "./EscolhaCliente";
import { AmbienteDoSistema } from "./PrepararAmbiente";
import type { ApiPrimeiroUso, SistemaCadastrado, Situacao } from "./primeiroUso";

export type Etapa = "cliente" | "base" | "sistema" | "ambiente" | "pronto";

const ETAPAS: { id: Etapa; rotulo: string }[] = [
  { id: "cliente", rotulo: "Cliente de IA" },
  { id: "base", rotulo: "Base de conhecimento" },
  { id: "sistema", rotulo: "Sistema" },
  { id: "ambiente", rotulo: "Ambiente do Claude" },
  { id: "pronto", rotulo: "Primeira análise" },
];

/** Perguntas que servem a qualquer sistema, para a primeira análise. */
export const SUGESTOES_PRIMEIRA_ANALISE = [
  "Quais são as principais regras de negócio deste sistema?",
  "Quais tabelas guardam os dados principais e como elas se relacionam?",
  "Quais validações são feitas antes de gravar um registro novo?",
];

/** O que o assistente entrega ao terminar. */
export interface FimPrimeiroUso {
  sistema: string;
  /** Pergunta escolhida para a primeira análise (vazia se o analista não quiser). */
  pergunta: string;
}

/**
 * Assistente do primeiro uso (spec, seção 4.1): cliente de IA, base de conhecimento,
 * cadastro do sistema com o inventário, preparo do ambiente com consentimento e a
 * sugestão da primeira análise. Cada passo é um componente próprio, reaproveitado
 * pelas configurações (#39).
 */
export function AssistentePrimeiroUso({
  api,
  situacao,
  aoConcluir,
}: {
  api: ApiPrimeiroUso;
  situacao: Situacao;
  aoConcluir: (fim: FimPrimeiroUso) => void;
}) {
  const [etapa, setEtapa] = useState<Etapa>("cliente");
  const [base, setBase] = useState<string | null>(situacao.base);
  const [sistema, setSistema] = useState<{ id: string; nome: string } | null>(null);
  const [cadastrado, setCadastrado] = useState<SistemaCadastrado | null>(null);
  const [ambientePronto, setAmbientePronto] = useState(false);
  const aoMudarAmbiente = useCallback((pronto: boolean) => setAmbientePronto(pronto), []);

  const indice = ETAPAS.findIndex((e) => e.id === etapa);

  return (
    <section className="assistente" aria-label="Primeiro uso">
      <h2 className="titulo-assistente">Vamos preparar o app</h2>
      <ol className="etapas" aria-label="Etapas">
        {ETAPAS.map((e, i) => (
          <li
            key={e.id}
            className={i === indice ? "atual" : i < indice ? "feita" : undefined}
            aria-current={i === indice ? "step" : undefined}
          >
            {e.rotulo}
          </li>
        ))}
      </ol>

      {etapa === "cliente" && (
        <EscolhaCliente api={api} inicial={situacao.cliente} aoConcluir={() => setEtapa("base")} />
      )}

      {etapa === "base" && (
        <EscolhaBase
          api={api}
          atual={base}
          sugerida={situacao.baseSugerida}
          aoConcluir={(pasta) => {
            setBase(pasta);
            setEtapa("sistema");
          }}
        />
      )}

      {etapa === "sistema" && (
        <>
          {situacao.sistemas.length > 0 && (
            <SistemaExistente
              sistemas={situacao.sistemas}
              aoEscolher={(s) => {
                setSistema(s);
                setCadastrado(null);
                setEtapa("ambiente");
              }}
            />
          )}
          <CadastroSistema
            api={api}
            aoConcluir={(s) => {
              setSistema({ id: s.id, nome: s.nome });
              setCadastrado(s);
              setEtapa("ambiente");
            }}
          />
        </>
      )}

      {etapa === "ambiente" && sistema && (
        <section className="passo-assistente" aria-label="Ambiente do Claude">
          <h2>Preparar o ambiente do Claude</h2>
          {cadastrado && (
            <details className="cadastrado">
              <summary>
                “{cadastrado.nome}” cadastrado: {cadastrado.inventario.analisados} arquivos para
                analisar.
              </summary>
              <ResumoDoInventario inventario={cadastrado.inventario} />
            </details>
          )}
          <p>
            Para o Claude consultar o sistema e registrar a análise no app, ele precisa conhecer o
            servidor do app (MCP) e avisar o app do andamento da conversa (hooks). O app mostra
            exatamente o que vai mudar na configuração do Claude e só grava com a sua concordância.
          </p>
          <AmbienteDoSistema api={api} sistema={sistema.id} aoMudar={aoMudarAmbiente} />
          <div className="acoes">
            <button type="button" onClick={() => setEtapa("pronto")} disabled={ambientePronto}>
              Preparar depois
            </button>
            <button
              type="button"
              className="primario"
              onClick={() => setEtapa("pronto")}
              disabled={!ambientePronto}
            >
              Continuar
            </button>
          </div>
        </section>
      )}

      {etapa === "pronto" && sistema && (
        <Pronto
          api={api}
          sistema={sistema}
          ambientePronto={ambientePronto}
          aoConcluir={aoConcluir}
        />
      )}

      {indice > 0 && etapa !== "pronto" && (
        <p>
          <button type="button" className="link" onClick={() => setEtapa(ETAPAS[indice - 1].id)}>
            ← Voltar
          </button>
        </p>
      )}
    </section>
  );
}

function SistemaExistente({
  sistemas,
  aoEscolher,
}: {
  sistemas: { id: string; nome: string }[];
  aoEscolher: (s: { id: string; nome: string }) => void;
}) {
  const [id, setId] = useState(sistemas[0].id);
  return (
    <div className="sistema-existente" role="group" aria-label="Sistema já cadastrado">
      <label>
        Usar um sistema já cadastrado{" "}
        <select value={id} onChange={(e) => setId(e.target.value)}>
          {sistemas.map((s) => (
            <option key={s.id} value={s.id}>
              {s.nome}
            </option>
          ))}
        </select>
      </label>{" "}
      <button type="button" onClick={() => aoEscolher(sistemas.find((s) => s.id === id)!)}>
        Usar este
      </button>
      <p className="suave">Ou cadastre outro:</p>
    </div>
  );
}

function Pronto({
  api,
  sistema,
  ambientePronto,
  aoConcluir,
}: {
  api: Pick<ApiPrimeiroUso, "concluir">;
  sistema: { id: string; nome: string };
  ambientePronto: boolean;
  aoConcluir: (fim: FimPrimeiroUso) => void;
}) {
  const [pergunta, setPergunta] = useState(SUGESTOES_PRIMEIRA_ANALISE[0]);
  const [falha, setFalha] = useState<string | null>(null);
  const [gravando, setGravando] = useState(false);

  const concluir = (comPergunta: boolean) => {
    setGravando(true);
    setFalha(null);
    api
      .concluir()
      .then(() => aoConcluir({ sistema: sistema.id, pergunta: comPergunta ? pergunta.trim() : "" }))
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setGravando(false);
      });
  };

  return (
    <section className="passo-assistente" aria-label="Primeira análise">
      <h2>Tudo pronto para analisar “{sistema.nome}”</h2>
      {!ambientePronto && (
        <p className="pendencia" role="status">
          O ambiente do Claude ainda não foi preparado. A tela da análise vai pedir o preparo antes
          da primeira pergunta.
        </p>
      )}
      <p>Que tal começar com uma pergunta? Escolha uma sugestão ou escreva a sua.</p>
      <div className="sugestoes" role="group" aria-label="Sugestões">
        {SUGESTOES_PRIMEIRA_ANALISE.map((s) => (
          <button key={s} type="button" onClick={() => setPergunta(s)}>
            {s}
          </button>
        ))}
      </div>
      <label>
        Primeira pergunta
        <textarea rows={3} value={pergunta} onChange={(e) => setPergunta(e.target.value)} />
      </label>
      {falha && (
        <p className="trecho-erro" role="alert">
          {falha}
        </p>
      )}
      <div className="acoes">
        <button type="button" onClick={() => concluir(false)} disabled={gravando}>
          Ir para o início
        </button>
        <button
          type="button"
          className="primario"
          onClick={() => concluir(true)}
          disabled={gravando || !pergunta.trim()}
        >
          Fazer a primeira análise
        </button>
      </div>
    </section>
  );
}
