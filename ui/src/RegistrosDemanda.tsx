import { useCallback, useEffect, useId, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { mensagemDe } from "./analise";
import {
  Alerta,
  BotaoIcone,
  EstadoVazio,
  ItemLista,
  ListaItens,
  MenuAcoes,
  Secao,
  Selo,
  TextoRico,
  type TomSelo,
} from "./componentes";
import { IconeConfirmar, IconeDescartar } from "./icones";
import {
  CATEGORIAS,
  NOME_CATEGORIA,
  ROTULO_NIVEL,
  ROTULO_RESTRICAO,
  ROTULO_STATUS_PREMISSA,
  TITULO_CATEGORIA,
  apiRegistrosTauri,
  listaPorExtenso,
  rotuloFonte,
  rotuloReferencia,
  textoCompleto,
  type ApiRegistros,
  type Categoria,
  type DadosRegistro,
  type Nivel,
  type PessoaVista,
  type Proposta,
  type RegistroVista,
  type RegistrosDaDemanda,
  type StatusPremissa,
  type TipoRestricao,
} from "./registros";
import "./registros.css";

const NIVEIS: Nivel[] = ["baixo", "medio", "alto"];
const RESTRICOES: TipoRestricao[] = ["prazo", "legal", "tecnica"];

const TITULO = "Premissas, decisões, riscos e restrições";

/** Linhas do texto de um registro antes do "ver mais". */
const LINHAS_DO_TEXTO = 4;

const QUEM: Record<Categoria, string> = {
  premissa: "Quem assumiu",
  decisao: "Quem decidiu",
  risco: "Responsável",
  restricao: "Quem informou",
};

const TOM_STATUS: Record<StatusPremissa, TomSelo> = {
  "nao-validada": "atencao",
  validada: "concluido",
  invalidada: "erro",
};

/** "Nova premissa", "Novo risco". */
const novo = (c: Categoria) => `${c === "risco" ? "Novo" : "Nova"} ${NOME_CATEGORIA[c]}`;

function riscoAlto(r: { probabilidade?: Nivel | null; impacto?: Nivel | null }) {
  return r.probabilidade === "alto" || r.impacto === "alto";
}

/** Lápis do "Ajustar" e do "Editar", no traço de `icones.tsx`. */
function IconeAjustar() {
  return (
    <svg
      width={16}
      height={16}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M12 20h9" />
      <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z" />
    </svg>
  );
}

/** Formulário aberto: criar, editar, ajustar uma proposta ou validar uma premissa. */
type Edicao =
  | { tipo: "nova"; categoria: Categoria }
  | { tipo: "editar"; registro: RegistroVista }
  | { tipo: "proposta"; proposta: Proposta }
  | { tipo: "validar"; registro: RegistroVista; status: StatusPremissa };

function dadosDoRegistro(r: RegistroVista): DadosRegistro {
  return {
    titulo: r.titulo,
    detalhe: r.detalhe,
    quem: r.quem,
    pessoa: r.pessoa,
    status: r.status,
    comoValidada: r.validacao?.como ?? null,
    decididoEm: r.decididoEm,
    alternativas: r.alternativas,
    probabilidade: r.probabilidade,
    impacto: r.impacto,
    resposta: r.resposta,
    restricao: r.restricao,
  };
}

function vazio(categoria: Categoria): DadosRegistro {
  return {
    titulo: "",
    status: categoria === "premissa" ? "nao-validada" : null,
    probabilidade: categoria === "risco" ? "medio" : null,
    impacto: categoria === "risco" ? "medio" : null,
    restricao: categoria === "restricao" ? "prazo" : null,
    alternativas: [],
  };
}

/** Quantos itens (registros e propostas) a categoria tem. */
function contagem(dados: RegistrosDaDemanda, c: Categoria): number {
  return (
    dados.registros.filter((r) => r.categoria === c).length +
    dados.propostas.filter((p) => p.categoria === c).length
  );
}

/** A aba que abre: a primeira com itens (sem itens, a de premissas). */
function abaInicial(dados: RegistrosDaDemanda): Categoria {
  return CATEGORIAS.find((c) => contagem(dados, c) > 0) ?? CATEGORIAS[0];
}

/**
 * Premissas, decisões, riscos e restrições da demanda (#122), no painel da demanda,
 * em abas por tipo com a contagem (#409; abre na primeira com itens). Em cada aba,
 * as propostas do agente (das análises de impacto e das atas) com o aviso, a seleção
 * e o "Aceitar as selecionadas", e cada uma com aceitar, ajustar e recusar; depois
 * os registros: as premissas com status, quem assumiu, a origem e os botões de
 * validar; as decisões com quem decidiu, a data, o item da ata e as alternativas;
 * os riscos numa matriz probabilidade × impacto; e as restrições por tipo. O texto
 * sai inteiro, em Markdown, recolhido em 4 linhas com "ver mais". Os registros ficam
 * na pasta da demanda, em Markdown com frontmatter.
 */
export function RegistrosDemanda({
  projeto,
  demanda,
  api = apiRegistrosTauri,
  atualizacao,
  abrirDocumento,
  podeEditar = true,
}: {
  projeto: string;
  demanda: string;
  api?: ApiRegistros;
  /** Muda quando o painel relê a base: a aba relê junto. */
  atualizacao?: unknown;
  abrirDocumento?: (caminho: string) => void;
  podeEditar?: boolean;
}) {
  const [dados, setDados] = useState<RegistrosDaDemanda | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  const [edicao, setEdicao] = useState<Edicao | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [ativa, setAtiva] = useState<Categoria | null>(null);
  const [selecionadas, setSelecionadas] = useState<ReadonlySet<string>>(new Set());
  const base = useId();
  const abas = useRef<Partial<Record<Categoria, HTMLButtonElement | null>>>({});

  // Outra demanda: a aba volta a abrir na primeira com itens, sem seleção.
  useEffect(() => {
    setAtiva(null);
    setSelecionadas(new Set());
  }, [projeto, demanda]);

  const carregar = useCallback(
    () =>
      api
        .registros(projeto, demanda)
        .then((r) => {
          if (r && Array.isArray(r.registros)) {
            setDados(r);
            setFalha(null);
            setAtiva((a) => a ?? abaInicial(r));
            // A seleção guarda só as propostas que ainda esperam o analista.
            setSelecionadas((s) => {
              const abertas = new Set(r.propostas.map((p) => p.referencia));
              return new Set([...s].filter((ref) => abertas.has(ref)));
            });
          }
        })
        .catch((e: unknown) => setFalha(mensagemDe(e))),
    [api, projeto, demanda],
  );

  useEffect(() => {
    void carregar();
  }, [carregar, atualizacao]);

  const executar = (acao: () => Promise<unknown>) => {
    setOcupado(true);
    setFalha(null);
    acao()
      .then(() => {
        setEdicao(null);
        void carregar();
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  /** A gravação de um aceite: a mesma no aceite individual, no ajuste e no lote. */
  const gravarAceite = (p: Proposta, ajustes?: DadosRegistro) =>
    api.aceitarProposta(projeto, demanda, p.referencia, ajustes);

  const aceitar = (p: Proposta, ajustes?: DadosRegistro) => executar(() => gravarAceite(p, ajustes));
  const recusar = (p: Proposta) =>
    executar(() => api.recusarProposta(projeto, demanda, p.referencia));

  /**
   * Aceita as propostas selecionadas, uma de cada vez e como o aceite individual
   * (sem ajustes). Uma que falha não para as outras: as aceitas saem da seleção, as
   * que falharam ficam selecionadas e o erro diz quais foram.
   */
  const aceitarEmLote = (propostas: Proposta[]) => {
    if (propostas.length === 0) return;
    setOcupado(true);
    setFalha(null);
    void (async () => {
      const falhas: string[] = [];
      for (const p of propostas) {
        try {
          await gravarAceite(p);
          setSelecionadas((s) => {
            const n = new Set(s);
            n.delete(p.referencia);
            return n;
          });
        } catch (e) {
          falhas.push(`${p.referencia}: ${mensagemDe(e)}`);
        }
      }
      // Relê antes de avisar: a releitura limpa o erro anterior.
      await carregar();
      if (falhas.length > 0) {
        const aceitas = propostas.length - falhas.length;
        setFalha(
          `${aceitas} de ${propostas.length} propostas aceitas. Não foi possível aceitar ${falhas.join("; ")}`,
        );
      }
      setOcupado(false);
    })();
  };

  if (!dados) {
    return (
      <Secao titulo={TITULO} nivel={3} className="registros-demanda">
        {falha ? <Alerta tom="erro" titulo={falha} /> : <p className="muted">Carregando…</p>}
      </Secao>
    );
  }

  const aba = ativa ?? abaInicial(dados);
  const registrosDa = (c: Categoria) => dados.registros.filter((r) => r.categoria === c);
  const propostasDa = (c: Categoria) => dados.propostas.filter((p) => p.categoria === c);
  const registros = registrosDa(aba);
  const propostas = propostasDa(aba);
  const escolhidas = propostas.filter((p) => selecionadas.has(p.referencia));
  const todasEscolhidas = propostas.length > 0 && escolhidas.length === propostas.length;

  const criar = (c: Categoria) => {
    setAtiva(c);
    setEdicao({ tipo: "nova", categoria: c });
  };

  const alternar = (referencias: string[], marcar: boolean) =>
    setSelecionadas((s) => {
      const n = new Set(s);
      for (const ref of referencias) {
        if (marcar) n.add(ref);
        else n.delete(ref);
      }
      return n;
    });

  const salvar = (d: DadosRegistro) => {
    if (!edicao) return;
    switch (edicao.tipo) {
      case "nova":
        return executar(() => api.criar(projeto, demanda, edicao.categoria, d));
      case "editar":
        return executar(() => api.editar(projeto, edicao.registro.id, d));
      case "proposta":
        return aceitar(edicao.proposta, d);
    }
  };

  const idAba = (c: Categoria) => `${base}-aba-${c}`;
  const idPainel = (c: Categoria) => `${base}-painel-${c}`;

  // Tablist com ativação automática: setas, Home e End trocam a aba e levam o foco.
  const teclado = (e: KeyboardEvent<HTMLButtonElement>) => {
    const atual = CATEGORIAS.indexOf(aba);
    const n = CATEGORIAS.length;
    const destino =
      e.key === "ArrowRight"
        ? (atual + 1) % n
        : e.key === "ArrowLeft"
          ? (atual - 1 + n) % n
          : e.key === "Home"
            ? 0
            : e.key === "End"
              ? n - 1
              : -1;
    if (destino < 0) return;
    e.preventDefault();
    const nova = CATEGORIAS[destino];
    setAtiva(nova);
    abas.current[nova]?.focus();
  };

  const fontes = listaPorExtenso([...new Set(propostas.map((p) => rotuloFonte(p.fonte)))]);
  const n = propostas.length;

  return (
    <Secao
      titulo={TITULO}
      nivel={3}
      className="registros-demanda"
      acoes={
        podeEditar && (
          <MenuAcoes
            rotulo="Novo registro"
            itens={CATEGORIAS.map((c) => ({ texto: novo(c), aoEscolher: () => criar(c) }))}
          />
        )
      }
    >
      {falha && <Alerta tom="erro" titulo={falha} />}

      {edicao && edicao.tipo !== "validar" && (
        <FormRegistro
          key={
            edicao.tipo === "nova"
              ? `nova:${edicao.categoria}`
              : edicao.tipo === "editar"
                ? edicao.registro.id
                : edicao.proposta.referencia
          }
          categoria={
            edicao.tipo === "nova"
              ? edicao.categoria
              : edicao.tipo === "editar"
                ? edicao.registro.categoria
                : edicao.proposta.categoria
          }
          titulo={
            edicao.tipo === "nova"
              ? novo(edicao.categoria)
              : edicao.tipo === "editar"
                ? `Editar ${edicao.registro.id}`
                : `Ajustar a proposta ${edicao.proposta.referencia}`
          }
          inicial={
            edicao.tipo === "nova"
              ? vazio(edicao.categoria)
              : edicao.tipo === "editar"
                ? dadosDoRegistro(edicao.registro)
                : edicao.proposta.dados
          }
          botao={edicao.tipo === "proposta" ? "Aceitar" : "Salvar"}
          pessoas={dados.pessoas}
          ocupado={ocupado}
          aoSalvar={salvar}
          aoCancelar={() => setEdicao(null)}
        />
      )}
      {edicao?.tipo === "validar" && (
        <FormValidacao
          registro={edicao.registro}
          status={edicao.status}
          ocupado={ocupado}
          aoCancelar={() => setEdicao(null)}
          aoSalvar={(como) =>
            executar(() =>
              api.validarPremissa(projeto, edicao.registro.id, {
                status: edicao.status,
                como,
              }),
            )
          }
        />
      )}

      <div className="registros-abas" role="tablist" aria-label="Tipo de registro">
        {CATEGORIAS.map((c) => (
          <button
            key={c}
            ref={(el) => {
              abas.current[c] = el;
            }}
            id={idAba(c)}
            type="button"
            role="tab"
            className="registros-aba"
            aria-selected={aba === c}
            aria-controls={idPainel(c)}
            tabIndex={aba === c ? 0 : -1}
            onClick={() => setAtiva(c)}
            onKeyDown={teclado}
          >
            {TITULO_CATEGORIA[c]} <span className="registros-aba-contagem">{contagem(dados, c)}</span>
          </button>
        ))}
      </div>

      <div
        className="registros-painel"
        role="tabpanel"
        id={idPainel(aba)}
        aria-labelledby={idAba(aba)}
        tabIndex={0}
      >
        {n > 0 && (
          <Alerta
            tom="atencao"
            role="note"
            titulo={`${n} ${n > 1 ? "propostas" : "proposta"} do agente`}
            acoes={
              podeEditar && (
                <>
                  <button
                    type="button"
                    className="btn"
                    disabled={ocupado}
                    onClick={() =>
                      alternar(
                        propostas.map((p) => p.referencia),
                        !todasEscolhidas,
                      )
                    }
                  >
                    {todasEscolhidas ? "Limpar a seleção" : "Selecionar todas"}
                  </button>
                  <button
                    type="button"
                    className="btn"
                    disabled={ocupado || escolhidas.length === 0}
                    onClick={() => aceitarEmLote(escolhidas)}
                  >
                    Aceitar as selecionadas{escolhidas.length > 0 && ` (${escolhidas.length})`}
                  </button>
                </>
              )
            }
          >
            {n > 1 ? "vindas" : "vinda"} de {fontes}. Revise antes de valerem para a demanda.
          </Alerta>
        )}

        {aba === "risco" && registros.length > 0 && <MatrizRiscos riscos={registros} />}

        {n === 0 && registros.length === 0 ? (
          <EstadoVazio
            acao={podeEditar ? { texto: "Criar o primeiro", aoEscolher: () => criar(aba) } : undefined}
          >
            Nenhum registro deste tipo.
          </EstadoVazio>
        ) : (
          <ListaItens rotulo={TITULO_CATEGORIA[aba]}>
            {propostas.map((p) => (
              <ItemProposta
                key={p.referencia}
                proposta={p}
                podeEditar={podeEditar}
                ocupado={ocupado}
                selecionada={selecionadas.has(p.referencia)}
                selecionar={(marcar) => alternar([p.referencia], marcar)}
                aceitar={() => aceitar(p)}
                ajustar={() => setEdicao({ tipo: "proposta", proposta: p })}
                recusar={() => recusar(p)}
              />
            ))}
            {registros.map((r) => (
              <ItemRegistro
                key={r.id}
                registro={r}
                podeEditar={podeEditar}
                ocupado={ocupado}
                abrirDocumento={abrirDocumento}
                editar={() => setEdicao({ tipo: "editar", registro: r })}
                validar={(status) => {
                  if (status === "nao-validada") {
                    executar(() => api.validarPremissa(projeto, r.id, { status }));
                  } else {
                    setEdicao({ tipo: "validar", registro: r, status });
                  }
                }}
              />
            ))}
          </ListaItens>
        )}
      </div>
      <p className="muted registros-pasta">
        Registros em <code>{dados.pasta}/</code>
      </p>
    </Secao>
  );
}

/** O que o agente sugere além do texto: status, quem, probabilidade × impacto, tipo. */
function sugestao(p: Proposta): string | null {
  const d = p.dados;
  const partes: string[] = [];
  if (d.quem) partes.push(d.quem);
  if (d.status) partes.push(`sugere ${ROTULO_STATUS_PREMISSA[d.status]}`);
  if (d.probabilidade && d.impacto)
    partes.push(`sugere ${ROTULO_NIVEL[d.probabilidade]} × ${ROTULO_NIVEL[d.impacto]}`);
  if (d.restricao) partes.push(`sugere ${ROTULO_RESTRICAO[d.restricao]}`);
  return partes.length > 0 ? partes.join(" · ") : null;
}

function ItemProposta({
  proposta: p,
  podeEditar,
  ocupado,
  selecionada,
  selecionar,
  aceitar,
  ajustar,
  recusar,
}: {
  proposta: Proposta;
  podeEditar: boolean;
  ocupado: boolean;
  selecionada: boolean;
  selecionar: (marcar: boolean) => void;
  aceitar: () => void;
  ajustar: () => void;
  recusar: () => void;
}) {
  const origem = rotuloReferencia(p.referencia);
  const sugere = sugestao(p);
  return (
    <ItemLista
      acoes={
        podeEditar && (
          <>
            <BotaoIcone
              rotulo={`Aceitar ${p.referencia}`}
              dica="Aceitar"
              data-acao="aceitar"
              disabled={ocupado}
              onClick={aceitar}
            >
              <IconeConfirmar />
            </BotaoIcone>
            <BotaoIcone rotulo={`Ajustar ${p.referencia}`} dica="Ajustar" disabled={ocupado} onClick={ajustar}>
              <IconeAjustar />
            </BotaoIcone>
            <BotaoIcone
              rotulo={`Recusar ${p.referencia}`}
              dica="Recusar"
              tom="perigo"
              disabled={ocupado}
              onClick={recusar}
            >
              <IconeDescartar />
            </BotaoIcone>
          </>
        )
      }
    >
      <div className="registro-item" data-proposta={p.referencia}>
        {podeEditar && (
          <input
            type="checkbox"
            className="registro-selecao"
            aria-label={`Selecionar ${p.referencia}`}
            checked={selecionada}
            disabled={ocupado}
            onChange={(e) => selecionar(e.target.checked)}
          />
        )}
        <div className="registro-corpo">
          <TextoRico texto={textoCompleto(p.dados.titulo, p.dados.detalhe)} linhas={LINHAS_DO_TEXTO} />
          <div className="registro-meta">
            <Selo tom="agente">proposta do agente</Selo>
            {origem && <span>{origem}</span>}
            {sugere && <span>{sugere}</span>}
          </div>
        </div>
      </div>
    </ItemLista>
  );
}

function ItemRegistro({
  registro: r,
  podeEditar,
  ocupado,
  abrirDocumento,
  editar,
  validar,
}: {
  registro: RegistroVista;
  podeEditar: boolean;
  ocupado: boolean;
  abrirDocumento?: (caminho: string) => void;
  editar: () => void;
  validar: (status: StatusPremissa) => void;
}) {
  const origem = rotuloReferencia(r.referencia);
  const detalhes: string[] = [];
  if (r.categoria === "premissa" && r.quem) detalhes.push(`assumida por ${r.quem}`);
  if (r.categoria === "decisao" && r.quem) detalhes.push(`decidiu ${r.quem}`);
  if (r.categoria === "decisao" && r.decididoEm) detalhes.push(r.decididoEm);
  if (r.categoria === "risco" && r.probabilidade && r.impacto)
    detalhes.push(`${ROTULO_NIVEL[r.probabilidade]} × ${ROTULO_NIVEL[r.impacto]}`);
  if (r.categoria === "risco" && r.quem) detalhes.push(`resp. ${r.quem}`);
  if (r.categoria === "risco" && r.resposta) detalhes.push(r.resposta);
  if (r.categoria === "restricao" && r.quem) detalhes.push(`informada por ${r.quem}`);
  if (origem) detalhes.push(origem);
  if (r.categoria === "decisao" && r.alternativas.length > 0)
    detalhes.push(`alternativas: ${r.alternativas.join(", ")}`);
  if (r.validacao) detalhes.push(`${r.validacao.como} (${r.validacao.em})`);
  return (
    <ItemLista
      acoes={
        podeEditar && (
          <>
            {r.categoria === "premissa" && r.status !== "validada" && (
              <BotaoIcone
                rotulo={`Validar ${r.id}`}
                dica="Validar"
                data-acao="aceitar"
                disabled={ocupado}
                onClick={() => validar("validada")}
              >
                <IconeConfirmar />
              </BotaoIcone>
            )}
            {r.categoria === "premissa" && r.status !== "invalidada" && (
              <BotaoIcone
                rotulo={`Invalidar ${r.id}`}
                dica="Invalidar"
                tom="perigo"
                disabled={ocupado}
                onClick={() => validar("invalidada")}
              >
                <IconeDescartar />
              </BotaoIcone>
            )}
            <BotaoIcone rotulo={`Editar ${r.id}`} dica="Editar" disabled={ocupado} onClick={editar}>
              <IconeAjustar />
            </BotaoIcone>
          </>
        )
      }
    >
      <div className="registro-item" data-registro={r.id}>
        <div className="registro-corpo">
          <TextoRico texto={textoCompleto(r.titulo, r.detalhe)} linhas={LINHAS_DO_TEXTO} />
          <div className="registro-meta">
            <span className="mono">{r.id}</span>
            {r.categoria === "premissa" && r.status && (
              <Selo tom={TOM_STATUS[r.status]}>{ROTULO_STATUS_PREMISSA[r.status]}</Selo>
            )}
            {r.categoria === "risco" && riscoAlto(r) && <Selo tom="atencao">risco alto</Selo>}
            {r.categoria === "restricao" && r.restricao && (
              <Selo>{ROTULO_RESTRICAO[r.restricao]}</Selo>
            )}
            {detalhes.length > 0 && <span>{detalhes.join(" · ")}</span>}
            {abrirDocumento && (
              <button type="button" className="link" onClick={() => abrirDocumento(r.documento)}>
                abrir
              </button>
            )}
            {abrirDocumento && r.origem && (
              <button type="button" className="link" onClick={() => abrirDocumento(r.origem!)}>
                abrir a origem
              </button>
            )}
          </div>
        </div>
      </div>
    </ItemLista>
  );
}

/** Matriz probabilidade (linhas, alta → baixa) × impacto (colunas, baixo → alto). */
function MatrizRiscos({ riscos }: { riscos: RegistroVista[] }) {
  const linhas: Nivel[] = ["alto", "medio", "baixo"];
  return (
    <table className="matriz-riscos" aria-label="Matriz probabilidade × impacto">
      <thead>
        <tr>
          <th scope="col">probabilidade \ impacto</th>
          {NIVEIS.map((i) => (
            <th key={i} scope="col">
              {ROTULO_NIVEL[i]}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {linhas.map((p) => (
          <tr key={p}>
            <th scope="row">{ROTULO_NIVEL[p]}</th>
            {NIVEIS.map((i) => {
              const aqui = riscos.filter((r) => r.probabilidade === p && r.impacto === i);
              return (
                <td key={i} data-nivel={p === "alto" || i === "alto" ? "alto" : undefined}>
                  {aqui.length === 0 ? "–" : aqui.map((r) => r.id).join(" ")}
                </td>
              );
            })}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function FormRegistro({
  categoria,
  titulo,
  inicial,
  botao,
  pessoas,
  ocupado,
  aoSalvar,
  aoCancelar,
}: {
  categoria: Categoria;
  titulo: string;
  inicial: DadosRegistro;
  botao: string;
  pessoas: PessoaVista[];
  ocupado: boolean;
  aoSalvar: (d: DadosRegistro) => void;
  aoCancelar: () => void;
}) {
  const [d, setD] = useState<DadosRegistro>(inicial);
  const [alternativas, setAlternativas] = useState((inicial.alternativas ?? []).join("\n"));
  const mudar = (campo: Partial<DadosRegistro>) => setD((atual) => ({ ...atual, ...campo }));
  const enviar = (e: FormEvent) => {
    e.preventDefault();
    aoSalvar({
      ...d,
      alternativas: alternativas
        .split("\n")
        .map((a) => a.trim())
        .filter(Boolean),
    });
  };
  const rotuloTitulo: Record<Categoria, string> = {
    premissa: "Enunciado da premissa",
    decisao: "O que foi decidido",
    risco: "Descrição do risco",
    restricao: "Restrição",
  };
  return (
    <form className="form-registro" aria-label={titulo} onSubmit={enviar}>
      <h4>{titulo}</h4>
      <label>
        {rotuloTitulo[categoria]}
        <input
          value={d.titulo}
          onChange={(e) => mudar({ titulo: e.target.value })}
          required
          maxLength={300}
        />
      </label>
      <label>
        {QUEM[categoria]} (catálogo de pessoas)
        <select
          value={d.pessoa ?? ""}
          onChange={(e) => {
            const slug = e.target.value || null;
            const pessoa = pessoas.find((p) => p.slug === slug);
            mudar({ pessoa: slug, quem: pessoa ? pessoa.nome : d.quem });
          }}
        >
          <option value="">fora do catálogo</option>
          {pessoas.map((p) => (
            <option key={p.slug} value={p.slug}>
              {p.nome}
              {p.papel ? ` (${p.papel})` : ""}
            </option>
          ))}
        </select>
      </label>
      <label>
        {QUEM[categoria]} (nome)
        <input value={d.quem ?? ""} onChange={(e) => mudar({ quem: e.target.value })} />
      </label>
      {categoria === "premissa" && (
        <>
          <label>
            Status
            <select
              value={d.status ?? "nao-validada"}
              onChange={(e) => mudar({ status: e.target.value as StatusPremissa })}
            >
              {(Object.keys(ROTULO_STATUS_PREMISSA) as StatusPremissa[]).map((s) => (
                <option key={s} value={s}>
                  {ROTULO_STATUS_PREMISSA[s]}
                </option>
              ))}
            </select>
          </label>
          {d.status && d.status !== "nao-validada" && (
            <label>
              Como foi validada
              <input
                value={d.comoValidada ?? ""}
                onChange={(e) => mudar({ comoValidada: e.target.value })}
              />
            </label>
          )}
        </>
      )}
      {categoria === "decisao" && (
        <>
          <label>
            Quando
            <input
              type="date"
              value={d.decididoEm ?? ""}
              onChange={(e) => mudar({ decididoEm: e.target.value || null })}
            />
          </label>
          <label>
            Alternativas consideradas (uma por linha)
            <textarea value={alternativas} onChange={(e) => setAlternativas(e.target.value)} />
          </label>
        </>
      )}
      {categoria === "risco" && (
        <>
          <label>
            Probabilidade
            <select
              value={d.probabilidade ?? "medio"}
              onChange={(e) => mudar({ probabilidade: e.target.value as Nivel })}
            >
              {NIVEIS.map((n) => (
                <option key={n} value={n}>
                  {ROTULO_NIVEL[n]}
                </option>
              ))}
            </select>
          </label>
          <label>
            Impacto
            <select
              value={d.impacto ?? "medio"}
              onChange={(e) => mudar({ impacto: e.target.value as Nivel })}
            >
              {NIVEIS.map((n) => (
                <option key={n} value={n}>
                  {ROTULO_NIVEL[n]}
                </option>
              ))}
            </select>
          </label>
          <label>
            Resposta
            <input value={d.resposta ?? ""} onChange={(e) => mudar({ resposta: e.target.value })} />
          </label>
        </>
      )}
      {categoria === "restricao" && (
        <label>
          Tipo
          <select
            value={d.restricao ?? "prazo"}
            onChange={(e) => mudar({ restricao: e.target.value as TipoRestricao })}
          >
            {RESTRICOES.map((t) => (
              <option key={t} value={t}>
                {ROTULO_RESTRICAO[t]}
              </option>
            ))}
          </select>
        </label>
      )}
      <label>
        Detalhe
        <textarea value={d.detalhe ?? ""} onChange={(e) => mudar({ detalhe: e.target.value })} />
      </label>
      <div className="registro-acoes">
        <button type="submit" className="btn" disabled={ocupado || !d.titulo.trim()}>
          {botao}
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

function FormValidacao({
  registro,
  status,
  ocupado,
  aoSalvar,
  aoCancelar,
}: {
  registro: RegistroVista;
  status: StatusPremissa;
  ocupado: boolean;
  aoSalvar: (como: string) => void;
  aoCancelar: () => void;
}) {
  const [como, setComo] = useState("");
  const verbo = status === "validada" ? "Validar" : "Invalidar";
  return (
    <form
      className="form-registro"
      aria-label={`${verbo} ${registro.id}`}
      onSubmit={(e) => {
        e.preventDefault();
        aoSalvar(como);
      }}
    >
      <h4>
        {verbo} {registro.id}: {registro.titulo}
      </h4>
      <label>
        Como foi {status === "validada" ? "validada" : "invalidada"}
        <input
          value={como}
          onChange={(e) => setComo(e.target.value)}
          required
          placeholder="Confirmado pelo PO na reunião R-0002"
        />
      </label>
      <div className="registro-acoes">
        <button type="submit" className="btn" disabled={ocupado || !como.trim()}>
          {verbo}
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}
