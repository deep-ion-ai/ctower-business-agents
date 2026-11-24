import { useState, type FormEvent } from "react";
import { mensagemDe } from "./analise";
import type { ApiConhecimento } from "./conhecimento";
import type { DemandaVista } from "./demandas";

export type TipoDemanda = "bug" | "funcionalidade" | "melhoria";

/** Os tipos de demanda negocial, com o rótulo e o exemplo da descrição de cada um. */
export const TIPOS_DEMANDA: {
  id: TipoDemanda;
  nome: string;
  desc: string;
  rotulo: string;
  exemplo: string;
  tituloEx: string;
  funcionalidade: string;
}[] = [
  {
    id: "bug",
    nome: "Corrigir bug",
    desc: "Algo não funciona como deveria.",
    rotulo: "O que acontece e o que deveria acontecer",
    exemplo:
      "Ex.: Ao cancelar uma fatura parcelada, as parcelas seguintes continuam sendo cobradas. Deveriam ser canceladas junto.",
    tituloEx: "Ex.: Parcelas seguem cobradas após cancelar a fatura",
    funcionalidade: "Funcionalidade afetada",
  },
  {
    id: "funcionalidade",
    nome: "Nova funcionalidade",
    desc: "Algo que o sistema ainda não faz.",
    rotulo: "O que o negócio precisa e por quê",
    exemplo:
      "Ex.: Permitir que clientes PJ parcelem a fatura em até 3 vezes, para reduzir a inadimplência.",
    tituloEx: "Ex.: Parcelamento de fatura para PJ",
    funcionalidade: "Funcionalidade relacionada",
  },
  {
    id: "melhoria",
    nome: "Melhoria",
    desc: "Mudar algo que já existe.",
    rotulo: "O que muda e qual o ganho",
    exemplo:
      "Ex.: Mostrar o motivo da recusa do pagamento na tela da fatura, para o atendimento não precisar abrir chamado.",
    tituloEx: "Ex.: Motivo da recusa na tela da fatura",
    funcionalidade: "Funcionalidade afetada",
  },
];

/**
 * O corpo da demanda: o tipo e a funcionalidade em linhas próprias, antes do pedido.
 * Enquanto a demanda não tem campo de tipo na base, ele vai no texto (#350).
 */
export function descricaoDaDemanda(tipo: TipoDemanda, descricao: string, funcionalidade: string): string {
  const t = TIPOS_DEMANDA.find((x) => x.id === tipo);
  const linhas = [`**Tipo:** ${t?.nome ?? tipo}`];
  if (funcionalidade.trim()) linhas.push(`**Funcionalidade:** ${funcionalidade.trim()}`);
  return `${linhas.join("  \n")}\n\n${descricao.trim()}`;
}

/**
 * "Demanda Negocial" (#350): o ponto de entrada de um pedido do negócio (corrigir
 * um bug, criar uma funcionalidade, melhorar o que existe). Registra a demanda no
 * projeto; as análises partem dela.
 */
export function FormDemandaNegocial({
  api,
  projeto,
  aoRegistrar,
  aoCancelar,
}: {
  api: ApiConhecimento;
  projeto: string;
  aoRegistrar: (d: DemandaVista) => void;
  aoCancelar: () => void;
}) {
  const [tipo, setTipo] = useState<TipoDemanda | null>(null);
  const [titulo, setTitulo] = useState("");
  const [descricao, setDescricao] = useState("");
  const [funcionalidade, setFuncionalidade] = useState("");
  const [chave, setChave] = useState("");
  const [gravando, setGravando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  const atual = TIPOS_DEMANDA.find((t) => t.id === tipo);
  const falta = [
    !tipo && "o tipo de demanda",
    !titulo.trim() && "o título",
    !descricao.trim() && "a descrição",
  ].filter((x): x is string => !!x);
  const motivo =
    falta.length === 0
      ? null
      : `Para registrar, falta ${falta.length === 1 ? falta[0] : `${falta.slice(0, -1).join(", ")} e ${falta[falta.length - 1]}`}.`;

  const registrar = (ev: FormEvent) => {
    ev.preventDefault();
    if (motivo || !tipo || gravando) return;
    setGravando(true);
    setFalha(null);
    api
      .criarDemanda(projeto, {
        titulo: titulo.trim(),
        descricao: descricaoDaDemanda(tipo, descricao, funcionalidade),
        chave: chave.trim() || undefined,
      })
      .then(aoRegistrar)
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setGravando(false);
      });
  };

  return (
    <form className="nova-pergunta nova-analise" onSubmit={registrar} aria-label="Demanda Negocial">
      <fieldset className="tipos-demanda">
        <legend>Tipo de demanda</legend>
        <div className="tipos-demanda-opcoes">
          {TIPOS_DEMANDA.map((t) => (
            <label key={t.id} className="cartao-opcao" data-marcado={tipo === t.id ? "" : undefined}>
              <input
                type="radio"
                name="tipo-demanda"
                value={t.id}
                checked={tipo === t.id}
                onChange={() => setTipo(t.id)}
              />
              <strong>{t.nome}</strong>
              <span className="muted">{t.desc}</span>
            </label>
          ))}
        </div>
      </fieldset>
      <label>
        Título
        <input
          value={titulo}
          onChange={(e) => setTitulo(e.target.value)}
          placeholder={atual?.tituloEx ?? "Um nome curto para o pedido"}
        />
      </label>
      <label>
        {atual?.rotulo ?? "Descrição"}
        <textarea
          value={descricao}
          onChange={(e) => setDescricao(e.target.value)}
          rows={5}
          placeholder={atual?.exemplo ?? "Escolha o tipo de demanda acima."}
        />
      </label>
      <div className="campos-lado-a-lado">
        <label>
          <span>
            {atual?.funcionalidade ?? "Funcionalidade"} <span className="muted">(opcional)</span>
          </span>
          <input
            value={funcionalidade}
            onChange={(e) => setFuncionalidade(e.target.value)}
            placeholder="Ex.: Faturamento › Parcelamento"
          />
        </label>
        <label>
          <span>
            Issue <span className="muted">(opcional)</span>
          </span>
          <input
            value={chave}
            onChange={(e) => setChave(e.target.value)}
            placeholder="Ex.: PROJ-123 ou #142"
            spellCheck={false}
          />
        </label>
      </div>
      <p className="muted ajuda-campo">
        A demanda ganha um código (D-0001) e aparece em Demandas. Depois de registrar, você pode
        analisá-la daqui mesmo.
      </p>
      <div className="rodape-form">
        {motivo && (
          <p className="motivo-desabilitado" id="demanda-negocial-motivo">
            {motivo}
          </p>
        )}
        <div className="acoes">
          <button className="btn" type="button" onClick={aoCancelar}>
            Cancelar
          </button>
          <button
            type="submit"
            className="btn btn-primary"
            disabled={motivo !== null || gravando}
            aria-describedby={motivo ? "demanda-negocial-motivo" : undefined}
          >
            Registrar demanda
          </button>
        </div>
      </div>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </form>
  );
}
