import {
  useCallback,
  useEffect,
  useId,
  useState,
  type FormEvent,
  type ReactNode,
} from "react";
import { mensagemDe } from "./analise";
import {
  Alerta,
  Cartao,
  EstadoVazio,
  MenuAcoes,
  Secao,
  Selo,
  TextoRico,
} from "./componentes";
import {
  agrupar,
  apiPautaTauri,
  destinatarioDoValor,
  hojeLocal,
  origemCurta,
  valorDestinatario,
  type Agrupamento,
  type ApiPauta,
  type DestinoDaResposta,
  type FormatoPauta,
  type ItemDaPauta,
  type Pauta,
  type RespostaRegistrada,
} from "./pauta";
import { ROTULO_STATUS_PREMISSA, type StatusPremissa } from "./registros";
import "./pauta.css";

const ROTULO_AGRUPAMENTO: Record<Agrupamento, string> = {
  destinatario: "Pessoa",
  papel: "Papel",
  tema: "Tema",
};

/**
 * Quantas perguntas cada grupo mostra antes do "Mostrar as outras N" (prancheta
 * Demanda reorganizada). Sobrando só uma, ela aparece direto.
 */
export const PERGUNTAS_VISIVEIS = 3;

function idade(dias: number): string {
  if (dias === 0) return "aberta hoje";
  return dias === 1 ? "há 1 dia" : `há ${dias} dias`;
}

function quantas(n: number): string {
  return n === 1 ? "1 pergunta" : `${n} perguntas`;
}

/**
 * Pauta de perguntas pendentes (#123), no painel da demanda: as perguntas em aberto
 * das análises e as dúvidas das atas aprovadas, agrupadas por pessoa, papel ou tema,
 * com a origem, a idade (as antigas em destaque), o que o sistema faz hoje e o que
 * cada uma bloqueia. Perguntas iguais aparecem juntas sem perder as originais. O
 * analista escolhe o destinatário, copia o e-mail de um grupo, exporta a pauta e
 * registra a resposta, que fecha a pergunta em cada análise de origem e pode virar
 * regra (informada, sem evidência no código) ou validar uma premissa.
 *
 * Desenho (#408): cada pergunta é um cartão com o número, a pergunta em destaque e o
 * selo da idade; "O que o sistema faz hoje" fica recolhido e, aberto, sai em
 * Markdown; a origem é um link curto (`A-0011 · pergunta 1`), na mesma linha do
 * "Para" e do "Registrar resposta".
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

  const erro = falha && (
    <Alerta tom="erro" titulo="Não foi possível concluir.">
      {falha}
    </Alerta>
  );

  if (!pauta) {
    return (
      <Secao titulo="Pauta de perguntas" nivel={3} className="pauta-perguntas">
        {erro || <p className="muted">Carregando…</p>}
      </Secao>
    );
  }

  const grupos = agrupar(pauta, modo);
  const semPerguntas = ocupado || pauta.total === 0;
  return (
    <Secao
      rotulo="Pauta de perguntas"
      nivel={3}
      className="pauta-perguntas"
      titulo={
        <>
          Pauta de perguntas <Selo>{`${pauta.total} em aberto`}</Selo>
          {pauta.antigas > 0 && (
            <>
              {" "}
              <Selo tom="atencao">
                {`${pauta.antigas} antiga${pauta.antigas > 1 ? "s" : ""}`}
              </Selo>
            </>
          )}
        </>
      }
      acoes={
        <>
          <label className="pauta-agrupar">
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
          <MenuAcoes
            rotulo="Exportar"
            desabilitado={semPerguntas}
            itens={[
              { texto: "Markdown", aoEscolher: () => exportar("markdown") },
              { texto: "Word", aoEscolher: () => exportar("word") },
            ]}
          />
        </>
      }
    >
      {erro}
      {aviso && (
        <p className="muted" role="status">
          {aviso}
        </p>
      )}

      {pauta.total === 0 ? (
        <EstadoVazio>
          Nenhuma pergunta em aberto: as perguntas para o solicitante das
          análises e as dúvidas das atas aprovadas aparecem aqui.
        </EstadoVazio>
      ) : (
        grupos.map((g, i) => {
          const email = modo === "destinatario" ? pauta.emails[i] : undefined;
          return (
            <GrupoPauta
              key={`${modo}:${g.rotulo}`}
              rotulo={g.rotulo}
              itens={g.itens}
              aoCopiar={
                email
                  ? () =>
                      executar(
                        () => api.copiar(email),
                        () => setAviso(`E-mail para ${g.rotulo} copiado.`),
                      )
                  : undefined
              }
              ocupado={ocupado}
              renderizar={(item, n) => (
                <CartaoPergunta
                  key={item.perguntas.map((p) => p.referencia).join(",")}
                  numero={n}
                  item={item}
                  pauta={pauta}
                  ocupado={ocupado}
                  abrirOrigem={(p) =>
                    p.fonte.tipo === "analise"
                      ? abrirAnalise?.(p.fonte.id)
                      : abrirDocumento?.(p.documento)
                  }
                  aoDefinirDestinatario={(valor) =>
                    executar(() =>
                      api.definirDestinatario(
                        projeto,
                        item.perguntas.map((p) => p.referencia),
                        destinatarioDoValor(valor),
                      ),
                    )
                  }
                  aoResponder={() => setRespondendo(item)}
                  formulario={
                    respondendo === item && (
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
                    )
                  }
                />
              )}
            />
          );
        })
      )}
    </Secao>
  );
}

/**
 * Um grupo da pauta: o título, a quantidade e "Copiar como e-mail"; as primeiras
 * {@link PERGUNTAS_VISIVEIS} perguntas e "Mostrar as outras N" (um botão, com
 * `aria-expanded`, que fica no lugar e vira "Mostrar menos": o foco não se perde).
 */
function GrupoPauta({
  rotulo,
  itens,
  aoCopiar,
  ocupado,
  renderizar,
}: {
  rotulo: string;
  itens: ItemDaPauta[];
  aoCopiar?: () => void;
  ocupado: boolean;
  renderizar: (item: ItemDaPauta, numero: number) => ReactNode;
}) {
  const idLista = useId();
  const [todas, setTodas] = useState(false);
  const recolhe = itens.length > PERGUNTAS_VISIVEIS + 1;
  const visiveis = recolhe && !todas ? itens.slice(0, PERGUNTAS_VISIVEIS) : itens;
  const outras = itens.length - PERGUNTAS_VISIVEIS;
  return (
    <section aria-label={rotulo} className="pauta-grupo">
      <div className="pauta-grupo-cabecalho">
        <h4>{rotulo}</h4>
        <span className="pauta-grupo-qtd">{quantas(itens.length)}</span>
        {aoCopiar && (
          <button
            type="button"
            className="link"
            disabled={ocupado}
            onClick={aoCopiar}
          >
            Copiar como e-mail
          </button>
        )}
      </div>
      <ol id={idLista} className="pauta-itens">
        {visiveis.map((item, i) => renderizar(item, i + 1))}
      </ol>
      {recolhe && (
        <button
          type="button"
          className="link pauta-mais"
          aria-expanded={todas}
          aria-controls={idLista}
          onClick={() => setTodas((t) => !t)}
        >
          {todas ? "Mostrar menos" : `Mostrar as outras ${outras} perguntas`}
        </button>
      )}
    </section>
  );
}

/**
 * O cartão de uma pergunta (ou de perguntas iguais juntas): número, pergunta e
 * idade; "O que o sistema faz hoje" recolhido; a origem curta, o "Para" e o
 * "Registrar resposta" numa linha só. Numa decisão para o negócio (#452), o selo
 * "Decisão", as opções com o efeito de cada uma e a recomendação.
 */
function CartaoPergunta({
  numero,
  item,
  pauta,
  ocupado,
  abrirOrigem,
  aoDefinirDestinatario,
  aoResponder,
  formulario,
}: {
  numero: number;
  item: ItemDaPauta;
  pauta: Pauta;
  ocupado: boolean;
  abrirOrigem: (p: ItemDaPauta["perguntas"][number]) => void;
  aoDefinirDestinatario: (valor: string) => void;
  aoResponder: () => void;
  formulario: ReactNode;
}) {
  const idContexto = useId();
  const [contextoAberto, setContextoAberto] = useState(false);
  const contextos = [
    ...new Set(
      item.perguntas
        .map((p) => p.contexto?.trim())
        .filter((c): c is string => !!c),
    ),
  ];
  return (
    <Cartao
      como="li"
      className={[
        "pauta-item",
        item.antiga ? "pauta-item-antiga" : "",
        item.decisao ? "pauta-item-decisao" : "",
      ]
        .filter(Boolean)
        .join(" ")}
    >
      <div className="pauta-item-topo">
        <span className="pauta-numero" aria-hidden="true">
          {numero}
        </span>
        <p className="pauta-texto">{item.texto}</p>
        <span className="pauta-selos">
          {item.decisao && <Selo tom="neutro">Decisão</Selo>}
          <Selo tom={item.antiga ? "atencao" : undefined}>
            {idade(item.idadeDias)}
          </Selo>
          {item.perguntas.length > 1 && (
            <Selo>{`${item.perguntas.length} perguntas iguais`}</Selo>
          )}
        </span>
      </div>
      <div className="pauta-item-corpo">
        {item.decisao && (
          <div className="pauta-decisao">
            <ol className="pauta-decisao-opcoes" aria-label="Opções">
              {item.decisao.opcoes.map((o, i) => (
                <li key={i}>
                  <strong>{o.texto}</strong>
                  <span className="muted"> — efeito: {o.efeito}</span>
                </li>
              ))}
            </ol>
            {item.decisao.recomendacao && (
              <p className="pauta-decisao-recomendacao">
                <strong>Recomendação:</strong> {item.decisao.recomendacao}
              </p>
            )}
          </div>
        )}
        {contextos.length > 0 && (
          <div className="pauta-contexto">
            <button
              type="button"
              className="link pauta-alternar"
              aria-expanded={contextoAberto}
              aria-controls={contextoAberto ? idContexto : undefined}
              onClick={() => setContextoAberto((a) => !a)}
            >
              <span aria-hidden="true">{contextoAberto ? "▾" : "▸"}</span>{" "}
              O que o sistema faz hoje
            </button>
            {contextoAberto && (
              <div id={idContexto} className="pauta-contexto-texto">
                {contextos.map((c) => (
                  <TextoRico key={c} texto={c} />
                ))}
              </div>
            )}
          </div>
        )}
        <div className="pauta-rodape">
          <ul className="pauta-origens" aria-label="Origem">
            {item.perguntas.map((p) => (
              <li key={p.referencia}>
                <button
                  type="button"
                  className="link pauta-origem"
                  title={
                    p.fonte.tipo === "analise"
                      ? `${p.fonte.id}: ${p.fonte.titulo}`
                      : `Reunião de ${p.fonte.data}`
                  }
                  onClick={() => abrirOrigem(p)}
                >
                  {origemCurta(p)}
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
                onChange={(e) => aoDefinirDestinatario(e.target.value)}
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
              onClick={aoResponder}
            >
              Registrar resposta
            </button>
          </div>
        </div>
        {formulario}
      </div>
    </Cartao>
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
      {item.decisao && (
        <fieldset>
          <legend>Opção escolhida</legend>
          {item.decisao.opcoes.map((o, i) => (
            <label key={i} className="pauta-opcao">
              <input
                type="radio"
                name="opcao-da-decisao"
                checked={texto === o.texto}
                onChange={() => setTexto(o.texto)}
              />{" "}
              {o.texto}
            </label>
          ))}
        </fieldset>
      )}
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
            {origemCurta(p)}
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
