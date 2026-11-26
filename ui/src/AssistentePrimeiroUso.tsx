import { useCallback, useState } from "react";
import { mensagemDe } from "./analise";
import { AssistenteProjeto } from "./AssistenteProjeto";
import { Alerta, BarraAcoes, Secao, TituloSecao } from "./componentes";
import { ResumoDoInventario } from "./ResumoDoInventario";
import { EscolhaBase } from "./EscolhaBase";
import { EscolhaCliente } from "./EscolhaCliente";
import { AmbienteDoProjeto } from "./PrepararAmbiente";
import type { ApiPrimeiroUso, ProjetoCadastrado, Situacao } from "./primeiroUso";

export type Etapa = "cliente" | "base" | "projeto" | "ambiente" | "pronto";

const ETAPAS: { id: Etapa; rotulo: string }[] = [
  { id: "cliente", rotulo: "Cliente de IA" },
  { id: "base", rotulo: "Base de conhecimento" },
  { id: "projeto", rotulo: "Projeto" },
  { id: "ambiente", rotulo: "Ambiente do Claude" },
  { id: "pronto", rotulo: "Primeira análise" },
];

/** Perguntas que servem a qualquer projeto, para a primeira análise. */
export const SUGESTOES_PRIMEIRA_ANALISE = [
  "Quais são as principais regras de negócio deste projeto?",
  "Quais tabelas guardam os dados principais e como elas se relacionam?",
  "Quais validações são feitas antes de gravar um registro novo?",
];

/** O que o assistente entrega ao terminar. */
export interface FimPrimeiroUso {
  projeto: string;
  /** Pergunta escolhida para a primeira análise (vazia se o analista não quiser). */
  pergunta: string;
  /** O ambiente do Claude está preparado: a análise já pode começar (#227). */
  ambientePronto?: boolean;
}

/**
 * Assistente do primeiro uso (spec, seção 4.1): cliente de IA, base de conhecimento,
 * cadastro do projeto com o inventário, preparo do ambiente com consentimento e a
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
  const [projeto, setProjeto] = useState<{ id: string; nome: string } | null>(null);
  const [cadastrado, setCadastrado] = useState<ProjetoCadastrado | null>(null);
  const [ambientePronto, setAmbientePronto] = useState(false);
  const aoMudarAmbiente = useCallback((pronto: boolean) => setAmbientePronto(pronto), []);

  const indice = ETAPAS.findIndex((e) => e.id === etapa);

  return (
    <section className="assistente" aria-label="Primeiro uso">
      <TituloSecao>Vamos preparar o app</TituloSecao>
      <ol className="etapas" aria-label="Etapas">
        {ETAPAS.map((e, i) => (
          <li
            key={e.id}
            className={i === indice ? "atual" : i < indice ? "feita" : undefined}
            aria-current={i === indice ? "step" : undefined}
          >
            {e.rotulo}{" "}
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
            setEtapa("projeto");
          }}
        />
      )}

      {etapa === "projeto" && (
        <>
          {situacao.projetos.length > 0 && (
            <ProjetoExistente
              projetos={situacao.projetos}
              aoEscolher={(s) => {
                setProjeto(s);
                setCadastrado(null);
                setEtapa("ambiente");
              }}
            />
          )}
          <AssistenteProjeto
            api={api}
            aoConcluir={(s) => {
              // “Fazer depois”: segue para a primeira análise sem preparar o ambiente.
              setProjeto({ id: s.id, nome: s.nome });
              setCadastrado(s);
              setAmbientePronto(false);
              setEtapa("pronto");
            }}
            aoPrepararAmbiente={(s) => {
              setProjeto({ id: s.id, nome: s.nome });
              setCadastrado(s);
              setEtapa("ambiente");
            }}
          />
        </>
      )}

      {etapa === "ambiente" && projeto && (
        <Secao
          titulo="Ligar o agente a este projeto"
          rotulo="Ambiente do Claude"
          className="passo-primeiro-uso"
        >
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
            Para o Claude consultar o projeto e registrar a análise no app, o app precisa ligá-lo a
            este projeto. O app mostra exatamente o que vai mudar na configuração do Claude e só
            grava com a sua concordância.
          </p>
          <AmbienteDoProjeto api={api} projeto={projeto.id} aoMudar={aoMudarAmbiente} />
          {/* "Continuar" é a principal, mas leva o motivo do bloqueio (aria-describedby):
              entra como botão próprio, no fim da fileira. */}
          <BarraAcoes
            rotulo="Ações do ambiente"
            alinhamento="fim"
            secundarias={
              ambientePronto ? [] : [{ texto: "Preparar depois", aoEscolher: () => setEtapa("pronto") }]
            }
          >
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => setEtapa("pronto")}
              disabled={!ambientePronto}
              aria-describedby={ambientePronto ? undefined : "motivo-ambiente"}
            >
              Continuar
            </button>
          </BarraAcoes>
          {!ambientePronto && (
            <p id="motivo-ambiente" className="dica motivo-bloqueio">
              Continuar fica liberado depois de ligar o agente (use “Ver e preparar” acima). Se
              preferir, escolha “Preparar depois”.
            </p>
          )}
        </Secao>
      )}

      {etapa === "pronto" && projeto && (
        <Pronto
          api={api}
          projeto={projeto}
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

function ProjetoExistente({
  projetos,
  aoEscolher,
}: {
  projetos: { id: string; nome: string }[];
  aoEscolher: (s: { id: string; nome: string }) => void;
}) {
  const [id, setId] = useState(projetos[0].id);
  return (
    <div className="projeto-existente" role="group" aria-label="Projeto já cadastrado">
      <label>
        Usar um projeto já cadastrado{" "}
        <select value={id} onChange={(e) => setId(e.target.value)}>
          {projetos.map((s) => (
            <option key={s.id} value={s.id}>
              {s.nome}
            </option>
          ))}
        </select>
      </label>{" "}
      <button
        className="btn"
        type="button"
        onClick={() => aoEscolher(projetos.find((s) => s.id === id)!)}
      >
        Usar este
      </button>
      <p className="muted">Ou cadastre outro:</p>
    </div>
  );
}

function Pronto({
  api,
  projeto,
  ambientePronto,
  aoConcluir,
}: {
  api: Pick<ApiPrimeiroUso, "concluir">;
  projeto: { id: string; nome: string };
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
      .then(() =>
        aoConcluir({
          projeto: projeto.id,
          pergunta: comPergunta ? pergunta.trim() : "",
          ambientePronto,
        }),
      )
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setGravando(false);
      });
  };

  return (
    <Secao
      titulo={
        ambientePronto
          ? `Tudo pronto para analisar “${projeto.nome}”`
          : `“${projeto.nome}” está cadastrado, falta ligar o agente`
      }
      rotulo="Primeira análise"
      className="passo-primeiro-uso"
    >
      {!ambientePronto && (
        <Alerta tom="atencao" titulo="O agente ainda não foi ligado a este projeto.">
          Os primeiros passos na Visão geral mostram o que falta, e a tela da análise pede o preparo
          antes da primeira pergunta.
        </Alerta>
      )}
      <p>Que tal começar com uma pergunta? Escolha uma sugestão ou escreva a sua.</p>
      <BarraAcoes
        rotulo="Sugestões"
        secundarias={SUGESTOES_PRIMEIRA_ANALISE.map((s) => ({ texto: s, aoEscolher: () => setPergunta(s) }))}
      />
      <label>
        Primeira pergunta
        <textarea rows={3} value={pergunta} onChange={(e) => setPergunta(e.target.value)} />
      </label>
      {falha && (
        <Alerta tom="erro" titulo="Não deu para concluir o primeiro uso.">
          {falha}
        </Alerta>
      )}
      {/* A principal leva o motivo do bloqueio (aria-describedby): botão próprio, no fim. */}
      <BarraAcoes
        rotulo="Ações da primeira análise"
        alinhamento="fim"
        secundarias={[{ texto: "Ir para o início", aoEscolher: () => concluir(false), desabilitado: gravando }]}
      >
        <button
          type="button"
          className="btn btn-primary"
          onClick={() => concluir(true)}
          disabled={gravando || !pergunta.trim()}
          aria-describedby={pergunta.trim() ? undefined : "motivo-pergunta"}
        >
          Fazer a primeira análise
        </button>
      </BarraAcoes>
      {!pergunta.trim() && (
        <p id="motivo-pergunta" className="dica motivo-bloqueio">
          Escreva uma pergunta ou escolha uma sugestão acima para fazer a primeira análise.
        </p>
      )}
    </Secao>
  );
}
