import { useCallback, useEffect, useState, type FormEvent } from "react";
import {
  mensagemDe,
  ROTULO_ESTADO_ANALISE,
  type AmbienteClaude,
  type AnaliseCriada,
  type AnaliseResumo,
  type ApiAmbiente,
} from "./analise";
import { ROTULO_FLUXO } from "./conhecimento";
import { PrepararAmbiente } from "./PrepararAmbiente";
import { Alerta, TituloSecao } from "./componentes";
import {
  apiPrototipoTauri,
  AVISO_ENVIO_HOSPEDADO,
  ORIGENS_PROTOTIPO,
  ROTULO_FIDELIDADE,
  ROTULO_MODO,
  telasDoTexto,
  type ApiPrototipo,
  type Fidelidade,
  type ModoPrototipo,
  type PedidoPrototipo,
} from "./prototipo";
import type { ProjetoResumo } from "./terminal";
import { TokensProjeto } from "./TokensProjeto";

/** O que o formulário precisa para listar as origens e conferir o agente. */
export type ApiOrigensPrototipo = ApiAmbiente & {
  listarAnalises(projeto: string): Promise<AnaliseResumo[]>;
};

/**
 * Entrada opcional do analista, como os campos a guardam (#379). Para acrescentar uma
 * escolha, ponha o campo aqui, o valor padrão em {@link ENTRADA_PROTOTIPO_PADRAO}, o
 * controle em {@link CamposPrototipo} e a conversão em {@link pedidoDaEntrada}.
 */
export interface EntradaPrototipo {
  /** Uma tela por linha; vazio deixa o agente listar as telas afetadas. */
  telas: string;
  fidelidade: Fidelidade;
  fora: string;
  /** Como as telas são geradas (#383); o padrão é o HTML local. */
  modo: ModoPrototipo;
  /** O analista confirmou o envio para fora (só vale no modo hospedado). */
  confirmaEnvio: boolean;
}

export const ENTRADA_PROTOTIPO_PADRAO: EntradaPrototipo = {
  telas: "",
  fidelidade: "wireframe",
  fora: "",
  modo: "html",
  confirmaEnvio: false,
};

/** O modo hospedado só segue com a confirmação explícita do envio. */
export function entradaPronta(e: EntradaPrototipo): boolean {
  return e.modo !== "design_hospedado" || e.confirmaEnvio;
}

/** O pedido que vai para o comando, a partir dos campos. */
export function pedidoDaEntrada(e: EntradaPrototipo): PedidoPrototipo {
  const fora = e.fora.trim();
  const pedido: PedidoPrototipo = {
    telas: telasDoTexto(e.telas),
    fidelidade: e.fidelidade,
    fora: fora || undefined,
    modo: e.modo,
  };
  if (e.modo === "design_hospedado" && e.confirmaEnvio) {
    pedido.confirmacaoEnvio = { aviso: AVISO_ENVIO_HOSPEDADO };
  }
  return pedido;
}

const DESCRICAO_MODO: Record<ModoPrototipo, string> = {
  html: "O agente escreve o HTML de cada tela aqui no app. Nada sai do ambiente.",
  design_local:
    "O agente aplica o método de design do Claude (hierarquia visual, componentes, tokens) " +
    "rodando aqui. Nada sai do ambiente.",
  design_hospedado:
    "O agente usa a ferramenta de design hospedada do Claude e traz cada tela de volta ao " +
    "app. Dados do projeto saem do ambiente.",
};

const DESCRICAO_FIDELIDADE: Record<Fidelidade, string> = {
  wireframe: "Estrutura e conteúdo, sem a identidade visual.",
  alta_fidelidade: "Com a identidade visual do projeto (os tokens aprovados abaixo).",
};

/** Os campos do pedido do protótipo: telas, fidelidade e o que fica de fora. */
export function CamposPrototipo({
  valor,
  aoMudar,
}: {
  valor: EntradaPrototipo;
  aoMudar: (v: EntradaPrototipo) => void;
}) {
  const mudar = <K extends keyof EntradaPrototipo>(campo: K, v: EntradaPrototipo[K]) =>
    aoMudar({ ...valor, [campo]: v });
  return (
    <fieldset className="campos-prototipo">
      <legend>O que você quer no protótipo (opcional)</legend>
      <label>
        Telas
        <textarea
          value={valor.telas}
          onChange={(e) => mudar("telas", e.target.value)}
          rows={3}
          placeholder={"Uma por linha. Ex.:\nEmissão da fatura\nParcelamento da fatura"}
        />
      </label>
      <span className="ajuda-campo">
        Em branco, o agente lista as telas afetadas pela análise de origem.
      </span>
      <div role="radiogroup" aria-label="Fidelidade" className="opcoes-fidelidade">
        {(Object.keys(ROTULO_FIDELIDADE) as Fidelidade[]).map((f) => (
          <label key={f} title={DESCRICAO_FIDELIDADE[f]}>
            <input
              type="radio"
              name="fidelidade-prototipo"
              value={f}
              checked={valor.fidelidade === f}
              onChange={() => mudar("fidelidade", f)}
            />{" "}
            {ROTULO_FIDELIDADE[f]}
          </label>
        ))}
      </div>
      <span className="ajuda-campo">{DESCRICAO_FIDELIDADE[valor.fidelidade]}</span>
      <div role="radiogroup" aria-label="Como gerar as telas" className="opcoes-fidelidade">
        {(Object.keys(ROTULO_MODO) as ModoPrototipo[]).map((m) => (
          <label key={m} title={DESCRICAO_MODO[m]}>
            <input
              type="radio"
              name="modo-prototipo"
              value={m}
              checked={valor.modo === m}
              // Trocar de modo sempre pede a confirmação de novo.
              onChange={() => aoMudar({ ...valor, modo: m, confirmaEnvio: false })}
            />{" "}
            {ROTULO_MODO[m]}
          </label>
        ))}
      </div>
      <span className="ajuda-campo">{DESCRICAO_MODO[valor.modo]}</span>
      {valor.modo === "design_hospedado" && (
        <div role="note" aria-label="Envio para fora do ambiente">
          <Alerta tom="atencao" role="none" titulo="O que sai do ambiente:">
            {AVISO_ENVIO_HOSPEDADO} O código do projeto e as evidências não vão. Sem os tokens
            aprovados, vai só a estrutura das telas.
            <label className="aviso-envio-confirma">
              <input
                type="checkbox"
                checked={valor.confirmaEnvio}
                onChange={(e) => mudar("confirmaEnvio", e.target.checked)}
              />{" "}
              Confirmo o envio das telas, dos elementos e dos tokens do projeto para a ferramenta
              hospedada
            </label>
          </Alerta>
        </div>
      )}
      <label>
        O que fica de fora
        <input
          value={valor.fora}
          onChange={(e) => mudar("fora", e.target.value)}
          placeholder="Ex.: O relatório de recebíveis fica para depois."
        />
      </label>
    </fieldset>
  );
}

/**
 * Formulário do "Protótipo de telas" (#379), nos três pontos de entrada: o painel "Nova
 * análise", a própria análise aprovada (`origemFixa`) e o painel da demanda
 * (`demandaFixa`). A origem é obrigatória (impacto, explicação, pergunta ou histórias),
 * as aprovadas em destaque; a não aprovada só gera aviso. Junto, o cartão "Identidade
 * visual" com a revisão dos tokens CSS do projeto (#384), que não bloqueia o pedido.
 */
export function NovaPrototipo({
  api,
  apiPrototipo = apiPrototipoTauri,
  projetos,
  origemFixa,
  demandaFixa,
  aoCriar,
  aoCancelar,
  semTitulo = false,
}: {
  api: ApiOrigensPrototipo;
  apiPrototipo?: ApiPrototipo;
  projetos: ProjetoResumo[];
  /** A análise de onde o pedido saiu ("Prototipar telas" na análise). */
  origemFixa?: { id: string; titulo: string };
  /** Demanda (`D-0001`) do primeiro projeto de `projetos`. */
  demandaFixa?: string;
  aoCriar: (projeto: string, criada: AnaliseCriada) => void;
  aoCancelar: () => void;
  /** Sem o título do fluxo (quem chama já mostra o tipo escolhido). */
  semTitulo?: boolean;
}) {
  const [projeto, setProjeto] = useState(projetos[0]?.id ?? "");
  const [analises, setAnalises] = useState<AnaliseResumo[] | null>(null);
  const [origem, setOrigem] = useState(origemFixa?.id ?? "");
  const [entrada, setEntrada] = useState<EntradaPrototipo>(ENTRADA_PROTOTIPO_PADRAO);
  const [ambiente, setAmbiente] = useState<AmbienteClaude | null>(null);
  const [criando, setCriando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  // As análises que podem ser a origem: as da demanda primeiro (pelo painel dela), as
  // aprovadas antes das outras e, em cada grupo, as mais recentes.
  useEffect(() => {
    if (!projeto || origemFixa) return;
    let ativo = true;
    setAnalises(null);
    setOrigem("");
    api
      .listarAnalises(projeto)
      .then((lista) => {
        if (!ativo) return;
        const daDemanda = (a: AnaliseResumo) => Number(!!demandaFixa && a.demanda === demandaFixa);
        const candidatas = lista
          .filter((a) => ORIGENS_PROTOTIPO.has(a.fluxo))
          .sort(
            (a, b) =>
              daDemanda(b) - daDemanda(a) ||
              Number(b.estado === "aprovada") - Number(a.estado === "aprovada") ||
              b.atualizadaMs - a.atualizadaMs,
          );
        setAnalises(candidatas);
        setOrigem(candidatas[0]?.id ?? "");
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, projeto, origemFixa, demandaFixa]);

  const verificar = useCallback(() => {
    if (!projeto) return;
    setAmbiente(null);
    api
      .verificarAmbiente(projeto)
      .then(setAmbiente)
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, projeto]);

  useEffect(verificar, [verificar]);

  const escolhida = analises?.find((a) => a.id === origem);
  const aprovadas = analises?.filter((a) => a.estado === "aprovada") ?? [];
  const outras = analises?.filter((a) => a.estado !== "aprovada") ?? [];
  const pronto = ambiente?.pronto === true;
  const podeCriar = !!projeto && origem !== "" && entradaPronta(entrada);

  const criar = (ev: FormEvent) => {
    ev.preventDefault();
    if (!podeCriar) return;
    setCriando(true);
    setFalha(null);
    apiPrototipo
      .criarAnaliseDePrototipo(projeto, {
        origem,
        demanda: demandaFixa,
        pedido: pedidoDaEntrada(entrada),
      })
      .then((criada) => aoCriar(projeto, criada))
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setCriando(false);
      });
  };

  const opcao = (a: AnaliseResumo) => (
    <option key={a.id} value={a.id}>
      {a.id}: {a.titulo} ({ROTULO_ESTADO_ANALISE[a.estado]})
    </option>
  );

  return (
    <form className="nova-pergunta" onSubmit={criar} aria-label={ROTULO_FLUXO.prototipo}>
      {!semTitulo && <TituloSecao nivel={3}>{ROTULO_FLUXO.prototipo}</TituloSecao>}
      <p className="muted">
        O agente lê a análise de origem, lista as telas afetadas (novas ou alteradas) e registra
        os campos, as validações, as mensagens e as ações de cada tela, ligados às regras e aos
        critérios de aceite da origem. O que não tem fundamento fica &quot;a confirmar&quot;.
      </p>
      {demandaFixa && <p className="muted">O protótipo fica ligado à demanda {demandaFixa}.</p>}
      {!demandaFixa && !origemFixa && projetos.length > 1 && (
        <label>
          Projeto{" "}
          <select value={projeto} onChange={(e) => setProjeto(e.target.value)}>
            {projetos.map((s) => (
              <option key={s.id} value={s.id}>
                {s.nome}
              </option>
            ))}
          </select>
        </label>
      )}
      {ambiente && !pronto && (
        <PrepararAmbiente api={api} projeto={projeto} ambiente={ambiente} aoConcluir={verificar} />
      )}
      {origemFixa ? (
        <p>
          <span className="muted">Análise de origem</span>{" "}
          <strong>
            {origemFixa.id}: {origemFixa.titulo}
          </strong>
        </p>
      ) : (
        <label>
          Análise de origem
          <select
            value={origem}
            onChange={(e) => setOrigem(e.target.value)}
            disabled={analises === null || analises.length === 0}
          >
            {aprovadas.length > 0 && <optgroup label="Aprovadas">{aprovadas.map(opcao)}</optgroup>}
            {outras.length > 0 && (
              <optgroup label="Ainda não aprovadas">{outras.map(opcao)}</optgroup>
            )}
          </select>
        </label>
      )}
      {analises?.length === 0 && (
        <p className="muted">
          Nenhuma análise de impacto, explicação, pergunta ou histórias neste projeto ainda. Faça
          antes a análise de impacto da demanda: o protótipo parte dela.
        </p>
      )}
      <CamposPrototipo valor={entrada} aoMudar={setEntrada} />
      {projeto && <TokensProjeto api={apiPrototipo} projeto={projeto} />}
      <div className="acoes">
        <button
          type="submit"
          className="btn btn-primary"
          disabled={!pronto || criando || !podeCriar}
        >
          Prototipar as telas
        </button>
        <button className="btn" type="button" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
      {/* Abaixo dos botões: o aviso não empurra o botão principal (#219). */}
      {escolhida && escolhida.estado !== "aprovada" && (
        <Alerta tom="atencao" role="note" titulo={`A análise ${escolhida.id} ainda não foi aprovada:`}>
          o agente prefere as afirmações que você já confirmou e trata as demais como a confirmar.
        </Alerta>
      )}
      {ambiente && !pronto && (
        <p className="muted">Ligue o agente a este projeto antes de pedir o protótipo.</p>
      )}
      {!entradaPronta(entrada) && (
        <p className="muted">
          Para usar o design hospedado, confirme o envio. Ou escolha o HTML local ou o design local:
          nada sai do ambiente.
        </p>
      )}
      {falha && <Alerta tom="erro" titulo={falha} />}
    </form>
  );
}
