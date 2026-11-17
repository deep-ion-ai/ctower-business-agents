import { useCallback, useEffect, useState, type FormEvent } from "react";
import { mensagemDe } from "./analise";
import {
  agrupar,
  apiPautaTauri,
  destinatarioDoValor,
  hojeLocal,
  valorDestinatario,
  type Agrupamento,
  type ApiPauta,
  type DestinoDaResposta,
  type FormatoPauta,
  type ItemDaPauta,
  type Pauta,
  type PerguntaDaPauta,
  type RespostaRegistrada,
} from "./pauta";
import { ROTULO_STATUS_PREMISSA, type StatusPremissa } from "./registros";
import "./pauta.css";

const ROTULO_AGRUPAMENTO: Record<Agrupamento, string> = {
  destinatario: "Pessoa",
  papel: "Papel",
  tema: "Tema",
};

function origem(p: PerguntaDaPauta): string {
  return p.fonte.tipo === "analise"
    ? `${p.referencia} · ${p.fonte.titulo}`
    : `${p.referencia} · reunião de ${p.fonte.data}`;
}

function idade(dias: number): string {
  if (dias === 0) return "aberta hoje";
  return dias === 1 ? "há 1 dia" : `há ${dias} dias`;
}

/**
 * Pauta de perguntas pendentes (#123), no painel da demanda: as perguntas em aberto
 * das análises e as dúvidas das atas aprovadas, agrupadas por pessoa, papel ou tema,
 * com a origem, a idade (as antigas em destaque), o que o sistema faz hoje e o que
 * cada uma bloqueia. Perguntas iguais aparecem juntas sem perder as originais. O
 * analista escolhe o destinatário, copia o e-mail de um grupo, exporta a pauta e
 * registra a resposta, que fecha a pergunta em cada análise de origem e pode virar
 * regra (informada, sem evidência no código) ou validar uma premissa.
 */
export function PautaPerguntas({
  projeto,
  demanda,
  api = apiPautaTauri,
  atualizacao,
  abrirAnalise,
  abrirDocumento,
}: {
  projeto: string;
  /** Sem demanda, a pauta é do projeto. */
  demanda: string | null;
  api?: ApiPauta;
  /** Muda quando o painel relê a base: a pauta relê junto. */
  atualizacao?: unknown;
  abrirAnalise?: (id: string) => void;
  abrirDocumento?: (caminho: string) => void;
}) {
  const [pauta, setPauta] = useState<Pauta | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const [modo, setModo] = useState<Agrupamento>("destinatario");
  const [respondendo, setRespondendo] = useState<ItemDaPauta | null>(null);
  const [ocupado, setOcupado] = useState(false);

  const carregar = useCallback(() => {
    let ativo = true;
    api
      .pauta(projeto, demanda)
      .then((p) => {
        if (!ativo || !p || !Array.isArray(p.grupos)) return;
        setPauta(p);
        setFalha(null);
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, projeto, demanda]);

  useEffect(carregar, [carregar, atualizacao]);

  const executar = (
    acao: () => Promise<unknown>,
    depois?: (r: unknown) => void,
  ) => {
    setOcupado(true);
    setFalha(null);
    setAviso(null);
    acao()
      .then((r) => {
        depois?.(r);
        carregar();
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const exportar = (formato: FormatoPauta) =>
    executar(
      async () => {
        const destino = await api.escolherDestino(
          `pauta-${demanda ?? projeto}`,
          formato,
        );
        if (!destino) return null;
        return api.exportar(projeto, demanda, formato, destino);
      },
      (gravado) =>
        gravado && setAviso(`Pauta exportada em ${String(gravado)}.`),
    );

  const titulo = <h3>Pauta de perguntas</h3>;
  if (!pauta) {
    return (
      <section aria-label="Pauta de perguntas" className="pauta-perguntas">
        {titulo}
        {falha ? (
          <p className="error-text" role="alert">
            {falha}
          </p>
        ) : (
          <p className="muted">Carregando…</p>
        )}
      </section>
    );
  }

  const grupos = agrupar(pauta, modo);
  return (
    <section aria-label="Pauta de perguntas" className="pauta-perguntas">
      <header className="pauta-cabecalho">
        {titulo}
        <span className="badge">{pauta.total} em aberto</span>
        {pauta.antigas > 0 && (
          <span className="badge" data-tone="attention">
            {pauta.antigas} antiga{pauta.antigas > 1 ? "s" : ""}
          </span>
        )}
        <div className="pauta-acoes">
          <label>
            Agrupar por{" "}
            <select
              value={modo}
              onChange={(e) => setModo(e.target.value as Agrupamento)}
            >
              {(Object.keys(ROTULO_AGRUPAMENTO) as Agrupamento[]).map((m) => (
                <option key={m} value={m}>
                  {ROTULO_AGRUPAMENTO[m]}
                </option>
              ))}
            </select>
          </label>
          <button
            type="button"
            className="btn"
            disabled={ocupado || pauta.total === 0}
            onClick={() => exportar("markdown")}
          >
            Exportar Markdown
          </button>
          <button
            type="button"
            className="btn"
            disabled={ocupado || pauta.total === 0}
            onClick={() => exportar("word")}
          >
            Exportar Word
          </button>
        </div>
      </header>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      {aviso && (
        <p className="muted" role="status">
          {aviso}
        </p>
      )}

      {pauta.total === 0 ? (
        <p className="muted">
          Nenhuma pergunta em aberto: as perguntas para o solicitante das
          análises e as dúvidas das atas aprovadas aparecem aqui.
        </p>
      ) : (
        grupos.map((g, i) => (
          <section key={g.rotulo} aria-label={g.rotulo} className="pauta-grupo">
            <h4>
              {g.rotulo} <span className="badge">{g.itens.length}</span>
              {modo === "destinatario" && pauta.emails[i] && (
                <button
                  type="button"
                  className="link"
                  onClick={() =>
                    executar(
                      () => api.copiar(pauta.emails[i]),
                      () => setAviso(`E-mail para ${g.rotulo} copiado.`),
                    )
                  }
                >
                  copiar e-mail
                </button>
              )}
            </h4>
            <ul className="pauta-itens">
              {g.itens.map((item) => (
                <li
                  key={item.perguntas.map((p) => p.referencia).join(",")}
                  data-antiga={item.antiga || undefined}
                >
                  <p className="pauta-texto">
                    {item.texto}{" "}
                    <span
                      className="badge"
                      data-tone={item.antiga ? "attention" : undefined}
                    >
                      {idade(item.idadeDias)}
                    </span>
                    {item.perguntas.length > 1 && (
                      <span className="badge" data-tone="running">
                        {item.perguntas.length} perguntas iguais
                      </span>
                    )}
                  </p>
                  {[
                    ...new Set(
                      item.perguntas.map((p) => p.contexto).filter(Boolean),
                    ),
                  ].map((c) => (
                    <p key={c} className="pauta-contexto">
                      <strong>O que o sistema faz hoje:</strong> {c}
                    </p>
                  ))}
                  <ul className="pauta-origens">
                    {item.perguntas.map((p) => (
                      <li key={p.referencia}>
                        <button
                          type="button"
                          className="link"
                          onClick={() =>
                            p.fonte.tipo === "analise"
                              ? abrirAnalise?.(p.fonte.id)
                              : abrirDocumento?.(p.documento)
                          }
                        >
                          {origem(p)}
                        </button>
                        {p.texto !== item.texto && (
                          <span className="muted"> · “{p.texto}”</span>
                        )}
                        {p.bloqueia.length > 0 && (
                          <span className="muted">
                            {" "}
                            · bloqueia {p.bloqueia.join(", ")}
                          </span>
                        )}
                      </li>
                    ))}
                  </ul>
                  <div className="pauta-item-acoes">
                    <label>
                      Para{" "}
                      <select
                        aria-label={`Destinatário de ${item.perguntas[0].referencia}`}
                        value={valorDestinatario(item.destinatario)}
                        disabled={ocupado}
                        onChange={(e) =>
                          executar(() =>
                            api.definirDestinatario(
                              projeto,
                              item.perguntas.map((p) => p.referencia),
                              destinatarioDoValor(e.target.value),
                            ),
                          )
                        }
                      >
                        <option value="">sem destinatário</option>
                        <optgroup label="Pessoas">
                          {pauta.pessoas.map((p) => (
                            <option key={p.slug} value={`pessoa:${p.slug}`}>
                              {p.nome}
                              {p.papel ? ` (${p.papel})` : ""}
                            </option>
                          ))}
                        </optgroup>
                        <optgroup label="Papéis">
                          {pauta.papeis.map((p) => (
                            <option key={p} value={`papel:${p}`}>
                              {p}
                            </option>
                          ))}
                        </optgroup>
                      </select>
                    </label>
                    <button
                      type="button"
                      className="btn"
                      disabled={ocupado}
                      onClick={() => setRespondendo(item)}
                    >
                      Registrar resposta
                    </button>
                  </div>
                  {respondendo === item && (
                    <FormResposta
                      item={item}
                      pauta={pauta}
                      ocupado={ocupado}
                      aoCancelar={() => setRespondendo(null)}
                      aoEnviar={(resposta) =>
                        executar(
                          () => api.registrarResposta(projeto, resposta),
                          (r) => {
                            setRespondendo(null);
                            setAviso(resumo(r as RespostaRegistrada));
                          },
                        )
                      }
                    />
                  )}
                </li>
              ))}
            </ul>
          </section>
        ))
      )}
    </section>
  );
}

function resumo(r: RespostaRegistrada): string {
  const partes = [
    r.fechadas.length === 1
      ? `Resposta registrada; a pergunta ${r.fechadas[0]} foi fechada.`
      : `Resposta registrada; as perguntas ${r.fechadas.join(", ")} foram fechadas.`,
  ];
  if (r.regra)
    partes.push(
      `Regra ${r.regra.id} criada, informada e sem evidência no código.`,
    );
  if (r.premissa) partes.push(`Premissa ${r.premissa} atualizada.`);
  partes.push(...r.avisos);
  return partes.join(" ");
}

type TipoDestino = DestinoDaResposta["tipo"];

function FormResposta({
  item,
  pauta,
  ocupado,
  aoCancelar,
  aoEnviar,
}: {
  item: ItemDaPauta;
  pauta: Pauta;
  ocupado: boolean;
  aoCancelar: () => void;
  aoEnviar: (r: Parameters<ApiPauta["registrarResposta"]>[1]) => void;
}) {
  const pessoaInicial =
    item.destinatario?.tipo === "pessoa" ? item.destinatario.pessoa : "";
  const [texto, setTexto] = useState("");
  const [pessoa, setPessoa] = useState(pessoaInicial);
  const [por, setPor] = useState(
    pauta.pessoas.find((p) => p.slug === pessoaInicial)?.nome ?? "",
  );
  const [em, setEm] = useState(hojeLocal());
  const [fonte, setFonte] = useState("");
  const [fechar, setFechar] = useState<string[]>(
    item.perguntas.map((p) => p.referencia),
  );
  const [destino, setDestino] = useState<TipoDestino>("so-registrar");
  const [tituloRegra, setTituloRegra] = useState("");
  const [area, setArea] = useState("");
  const [premissa, setPremissa] = useState(pauta.premissas[0]?.id ?? "");
  const [status, setStatus] = useState<StatusPremissa>("validada");

  const enviar = (e: FormEvent) => {
    e.preventDefault();
    const d: DestinoDaResposta =
      destino === "regra"
        ? {
            tipo: "regra",
            titulo: tituloRegra.trim() || null,
            area: area.trim() || null,
          }
        : destino === "premissa"
          ? { tipo: "premissa", premissa, status }
          : { tipo: "so-registrar" };
    aoEnviar({
      texto,
      por,
      pessoa: pessoa || null,
      em: em || null,
      fonte,
      perguntas: fechar,
      destino: d,
    });
  };

  const alternar = (ref: string) =>
    setFechar((f) =>
      f.includes(ref) ? f.filter((x) => x !== ref) : [...f, ref],
    );

  return (
    <form
      className="pauta-resposta"
      aria-label="Registrar resposta"
      onSubmit={enviar}
    >
      <label>
        Resposta
        <textarea
          required
          value={texto}
          onChange={(e) => setTexto(e.target.value)}
          rows={3}
        />
      </label>
      <div className="pauta-linha">
        <label>
          Quem respondeu (catálogo)
          <select
            value={pessoa}
            onChange={(e) => {
              setPessoa(e.target.value);
              const p = pauta.pessoas.find((x) => x.slug === e.target.value);
              if (p) setPor(p.nome);
            }}
          >
            <option value="">outra pessoa</option>
            {pauta.pessoas.map((p) => (
              <option key={p.slug} value={p.slug}>
                {p.nome}
              </option>
            ))}
          </select>
        </label>
        <label>
          Nome
          <input
            required
            value={por}
            onChange={(e) => setPor(e.target.value)}
          />
        </label>
        <label>
          Data
          <input
            type="date"
            value={em}
            onChange={(e) => setEm(e.target.value)}
          />
        </label>
        <label>
          Fonte
          <input
            value={fonte}
            placeholder="reunião, e-mail, conversa"
            onChange={(e) => setFonte(e.target.value)}
          />
        </label>
      </div>
      <fieldset>
        <legend>Fechar a pergunta em</legend>
        {item.perguntas.map((p) => (
          <label key={p.referencia} className="pauta-opcao">
            <input
              type="checkbox"
              checked={fechar.includes(p.referencia)}
              onChange={() => alternar(p.referencia)}
            />{" "}
            {origem(p)}
          </label>
        ))}
      </fieldset>
      <fieldset>
        <legend>O que fazer com a resposta</legend>
        <label className="pauta-opcao">
          <input
            type="radio"
            name="destino"
            checked={destino === "so-registrar"}
            onChange={() => setDestino("so-registrar")}
          />{" "}
          Só registrar
        </label>
        <label className="pauta-opcao">
          <input
            type="radio"
            name="destino"
            checked={destino === "regra"}
            onChange={() => setDestino("regra")}
          />{" "}
          Virar regra de negócio
        </label>
        {destino === "regra" && (
          <div className="pauta-detalhe">
            <p className="pauta-aviso-regra" role="note">
              A regra entra na base marcada como informada por{" "}
              {por.trim() || "quem respondeu"}, sem evidência no código: é uma
              exceção visível ao princípio “sem evidência, não é fato”, até
              alguém verificar no código.
            </p>
            <label>
              Título da regra
              <input
                value={tituloRegra}
                placeholder="a primeira frase da resposta"
                onChange={(e) => setTituloRegra(e.target.value)}
              />
            </label>
            <label>
              Área
              <input
                value={area}
                placeholder="geral"
                onChange={(e) => setArea(e.target.value)}
              />
            </label>
          </div>
        )}
        <label className="pauta-opcao">
          <input
            type="radio"
            name="destino"
            checked={destino === "premissa"}
            disabled={pauta.premissas.length === 0}
            onChange={() => setDestino("premissa")}
          />{" "}
          Validar uma premissa da demanda
          {pauta.premissas.length === 0 && (
            <span className="muted"> (nenhuma premissa)</span>
          )}
        </label>
        {destino === "premissa" && (
          <div className="pauta-detalhe">
            <label>
              Premissa
              <select
                value={premissa}
                onChange={(e) => setPremissa(e.target.value)}
              >
                {pauta.premissas.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.id}: {p.titulo} ({ROTULO_STATUS_PREMISSA[p.status]})
                  </option>
                ))}
              </select>
            </label>
            <label>
              Resultado
              <select
                value={status}
                onChange={(e) => setStatus(e.target.value as StatusPremissa)}
              >
                <option value="validada">validada</option>
                <option value="invalidada">invalidada</option>
              </select>
            </label>
          </div>
        )}
      </fieldset>
      <div className="pauta-item-acoes">
        <button
          type="submit"
          className="btn btn-primary"
          disabled={
            ocupado || fechar.length === 0 || !texto.trim() || !por.trim()
          }
        >
          Registrar
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}
