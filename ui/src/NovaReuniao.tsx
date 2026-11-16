import { useCallback, useEffect, useState, type FormEvent } from "react";
import { mensagemDe, type AmbienteClaude, type AnaliseCriada } from "./analise";
import { ROTULO_FLUXO, type ApiConhecimento } from "./conhecimento";
import type { DemandaVista } from "./demandas";
import { PrepararAmbiente } from "./PrepararAmbiente";
import {
  apiReuniaoTauri,
  nomeDoArquivo,
  ROTULO_FORMATO,
  type ApiReuniao,
  type DemandaDaReuniao,
  type EntradaTranscript,
  type PreviaTranscript,
} from "./reunioes";
import type { ProjetoResumo } from "./terminal";

/** Valor do seletor de demanda que pede uma demanda nova. */
const NOVA = "__nova__";

/**
 * Nova reunião (#141): a demanda (uma aberta ou uma nova, com a chave externa
 * opcional), o transcript (texto colado ou `.vtt`/`.docx` anexado) com a prévia do
 * parser, e o aviso de privacidade, que precisa ser aceito antes de criar. Ao
 * confirmar, o app cria a reunião, guarda o transcript, monta o estado inicial da
 * ata e abre a análise do fluxo `reuniao`, como a NovaAnalise faz nos outros fluxos.
 */
export function NovaReuniao({
  api,
  apiReuniao = apiReuniaoTauri,
  projetos,
  demandaFixa,
  aoCriar,
  aoCancelar,
}: {
  api: ApiConhecimento;
  apiReuniao?: ApiReuniao;
  projetos: ProjetoResumo[];
  /** Demanda (`D-0001`) do primeiro projeto de `projetos` (pelo painel da demanda). */
  demandaFixa?: string;
  aoCriar: (projeto: string, criada: AnaliseCriada) => void;
  aoCancelar: () => void;
}) {
  const [projeto, setProjeto] = useState(projetos[0]?.id ?? "");
  const [abertas, setAbertas] = useState<DemandaVista[]>([]);
  const [demanda, setDemanda] = useState(demandaFixa ?? NOVA);
  const [titulo, setTitulo] = useState("");
  const [chave, setChave] = useState("");
  const [texto, setTexto] = useState("");
  const [arquivo, setArquivo] = useState<string | null>(null);
  const [previa, setPrevia] = useState<PreviaTranscript | null>(null);
  const [lendo, setLendo] = useState(false);
  const [aceito, setAceito] = useState(false);
  const [ambiente, setAmbiente] = useState<AmbienteClaude | null>(null);
  const [criando, setCriando] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  useEffect(() => {
    if (demandaFixa || !projeto) return;
    let ativo = true;
    setAbertas([]);
    api
      .listarDemandas(projeto)
      .then((l) => {
        if (!ativo) return;
        const a = l.filter((d) => d.status === "aberta");
        setAbertas(a);
        setDemanda(a[0]?.id ?? NOVA);
      })
      // Sem a lista, ainda dá para criar uma demanda nova.
      .catch(() => ativo && setDemanda(NOVA));
    return () => {
      ativo = false;
    };
  }, [api, projeto, demandaFixa]);

  const verificar = useCallback(() => {
    if (!projeto) return;
    setAmbiente(null);
    api
      .verificarAmbiente(projeto)
      .then(setAmbiente)
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api, projeto]);

  useEffect(verificar, [verificar]);

  const entrada: EntradaTranscript | null = arquivo
    ? { tipo: "arquivo", caminho: arquivo }
    : texto.trim()
      ? { tipo: "texto", texto }
      : null;

  const lerPrevia = (e: EntradaTranscript) => {
    setLendo(true);
    setFalha(null);
    apiReuniao
      .previaTranscript(e)
      .then(setPrevia)
      .catch((err: unknown) => setFalha(mensagemDe(err)))
      .finally(() => setLendo(false));
  };

  const mudarTexto = (t: string) => {
    setTexto(t);
    setPrevia(null);
  };

  const anexar = () => {
    setFalha(null);
    apiReuniao
      .escolherArquivo()
      .then((c) => {
        if (!c) return;
        setArquivo(c);
        setPrevia(null);
        lerPrevia({ tipo: "arquivo", caminho: c });
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  const tirarArquivo = () => {
    setArquivo(null);
    setPrevia(null);
  };

  const pedidoDemanda: DemandaDaReuniao | null =
    demandaFixa || demanda !== NOVA
      ? { tipo: "existente", id: demandaFixa ?? demanda }
      : titulo.trim()
        ? { tipo: "nova", titulo: titulo.trim(), chave: chave.trim() || null }
        : null;

  const pronto = ambiente?.pronto === true;
  const podeCriar =
    pronto && !criando && aceito && previa?.podeCriar === true && !!entrada && !!pedidoDemanda;

  const criar = (ev: FormEvent) => {
    ev.preventDefault();
    // O botão já fica desabilitado; sem o aviso aceito, nada é pedido ao app.
    if (!podeCriar || !entrada || !pedidoDemanda) return;
    setCriando(true);
    setFalha(null);
    apiReuniao
      .criarReuniao(projeto, pedidoDemanda, entrada, aceito)
      .then((criada) => aoCriar(projeto, criada))
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setCriando(false);
      });
  };

  return (
    <form className="nova-pergunta nova-reuniao" onSubmit={criar} aria-label={ROTULO_FLUXO.reuniao}>
      <h3>{ROTULO_FLUXO.reuniao}</h3>
      {demandaFixa ? (
        <p className="muted">A reunião fica na demanda {demandaFixa}.</p>
      ) : (
        <>
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
          <label>
            Demanda
            <select value={demanda} onChange={(e) => setDemanda(e.target.value)}>
              {abertas.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.id}: {d.titulo}
                </option>
              ))}
              <option value={NOVA}>Criar uma demanda nova</option>
            </select>
          </label>
          {demanda === NOVA && (
            <>
              <label>
                Título da demanda
                <input
                  value={titulo}
                  onChange={(e) => setTitulo(e.target.value)}
                  placeholder="Ex.: Cancelamento de fatura antiga"
                />
              </label>
              <label>
                Chave externa (opcional)
                <input
                  value={chave}
                  onChange={(e) => setChave(e.target.value)}
                  placeholder="Ex.: FAT-31"
                  spellCheck={false}
                />
              </label>
            </>
          )}
        </>
      )}
      {ambiente && !pronto && (
        <PrepararAmbiente api={api} projeto={projeto} ambiente={ambiente} aoConcluir={verificar} />
      )}

      {arquivo ? (
        <p>
          Arquivo: <code>{nomeDoArquivo(arquivo)}</code>{" "}
          <button type="button" className="btn btn-sm" onClick={tirarArquivo}>
            Tirar o arquivo
          </button>
        </p>
      ) : (
        <>
          <label>
            Transcript
            <textarea
              value={texto}
              onChange={(e) => mudarTexto(e.target.value)}
              rows={6}
              placeholder="Cole aqui o transcript copiado do Teams."
            />
          </label>
          <div className="acoes">
            <button type="button" className="btn" onClick={anexar}>
              Anexar .vtt ou .docx
            </button>
            <button
              type="button"
              className="btn"
              disabled={!entrada || lendo}
              onClick={() => entrada && lerPrevia(entrada)}
            >
              Ver a prévia
            </button>
          </div>
        </>
      )}
      {lendo && <p className="muted">Lendo o transcript…</p>}
      {previa && <Previa previa={previa} />}

      <section className="aviso-privacidade" aria-label="Aviso de privacidade">
        <h4>Aviso de privacidade</h4>
        <p>O transcript tem dados pessoais: nomes e falas dos participantes. Ao seguir:</p>
        <ul>
          <li>o conteúdo vai ao provedor de IA que você usa no Claude Code;</li>
          <li>
            fica guardado só no diretório de dados do app, fora da base de conhecimento e do
            repositório;
          </li>
          <li>
            é apagado ao aprovar a ata ou, sem aprovação, depois do prazo de retenção (30 dias,
            configurável). Depois disso, as citações da ata deixam de ser conferíveis.
          </li>
        </ul>
        <label className="aceite">
          <input type="checkbox" checked={aceito} onChange={(e) => setAceito(e.target.checked)} />{" "}
          Li o aviso e quero seguir
        </label>
      </section>

      <div className="acoes">
        <button type="submit" className="btn btn-primary" disabled={!podeCriar}>
          Criar a reunião e iniciar a análise
        </button>
        <button className="btn" type="button" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
      {ambiente && !pronto && (
        <p className="muted">Prepare o ambiente do Claude antes de iniciar a análise.</p>
      )}
      {!aceito && previa?.podeCriar && (
        <p className="muted">Aceite o aviso de privacidade para criar a reunião.</p>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </form>
  );
}

function Previa({ previa }: { previa: PreviaTranscript }) {
  return (
    <section className="previa-transcript" aria-label="Prévia do transcript">
      <h4>Prévia</h4>
      <p>
        {ROTULO_FORMATO[previa.formato]} · {previa.falas} fala(s) · {previa.falantes.length}{" "}
        falante(s)
        {previa.semFalante > 0 && ` · ${previa.semFalante} fala(s) sem falante`}
      </p>
      {previa.falantes.length > 0 && (
        <ul className="falantes">
          {previa.falantes.map((f) => (
            <li key={f.nome}>
              {f.nome} <span className="muted">· {f.falas} fala(s)</span>
            </li>
          ))}
        </ul>
      )}
      {previa.avisos.length > 0 && (
        <ul className="avisos-transcript" aria-label="Avisos da leitura">
          {previa.avisos.map((a) => (
            <li key={a.codigo} data-tone="attention">
              {a.mensagem}
            </li>
          ))}
        </ul>
      )}
      {!previa.podeCriar && (
        <p className="error-text">Nenhuma fala encontrada: confira o transcript.</p>
      )}
    </section>
  );
}
