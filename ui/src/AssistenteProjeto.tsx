import { useEffect, useRef, useState, type FormEvent, type ReactNode, type RefObject } from "react";
import { mensagemDe } from "./analise";
import {
  DEMANDAS_INICIAIS,
  ROTULO_FORJA,
  clonesPreenchidos,
  codigoDoProjeto,
  codigoInicial,
  hostsDistintos,
  linhas,
  nomeDaPasta,
  nomeDoProjeto,
  pastasDoProjeto,
  plural,
  rotuloRepositorio,
  todosClonados,
  trackerDoProjeto,
  type ApiAssistenteProjeto,
  type EstadoCodigo,
  type EstadoDemandas,
} from "./estadoCadastro";
import { PassoCodigo } from "./PassoCodigo";
import { PassoDemandas } from "./PassoDemandas";
import { PassoLeitura } from "./PassoLeitura";
import { useTokensGuardados } from "./useTokensGuardados";
import {
  padraoDoCaminho,
  tamanhoLegivel,
  type PedidoProjeto,
  type PreviaProjeto,
  type ProjetoCadastrado,
} from "./primeiroUso";
import {
  PROGRESSO_CLONE_INICIAL,
  alvoDoToken,
  hostDaUrl,
  reduzirProgressoClone,
  validarRepositorios,
  type EventoClone,
  type RepositorioCodigo,
} from "./repositorios";
import "./assistenteProjeto.css";

export type { ApiAssistenteProjeto } from "./estadoCadastro";

type Passo = 1 | 2 | 3 | 4 | 5;

const ETAPAS: { n: Exclude<Passo, 5>; rotulo: string; sub: string }[] = [
  { n: 1, rotulo: "Código", sub: "clonar ou escolher pasta" },
  { n: 2, rotulo: "O que será lido", sub: "prévia automática" },
  { n: 3, rotulo: "Demandas", sub: "opcional" },
  { n: 4, rotulo: "Revisar", sub: "e cadastrar" },
];

const TITULOS: Record<Exclude<Passo, 5>, { titulo: string; texto: string }> = {
  1: {
    titulo: "De onde vem o código do projeto?",
    texto:
      "Clone os repositórios por aqui ou aponte uma pasta que você já tem. O app só lê o código, nunca grava nada nele.",
  },
  2: {
    titulo: "É isto que a IA vai ler",
    texto:
      "O app já separou o que interessa. Dependências, binários e arquivos que parecem ter segredos ficam fora.",
  },
  3: {
    titulo: "Conectar as demandas (opcional)",
    texto:
      "Com as issues conectadas, as demandas vêm de lá. Dá para fazer isso depois, em Configurações.",
  },
  4: {
    titulo: "Tudo certo?",
    texto: "Confira antes de cadastrar. Depois, tudo isso pode ser mudado em Configurações.",
  },
};

/** A prévia vale para estes campos do pedido (os repositórios não mudam o inventário). */
function chaveDoPedido(p: PedidoProjeto): string {
  return JSON.stringify([p.nome, p.pastas, p.exclusoes, p.inclusoes]);
}

/**
 * Assistente de cadastro de projeto (#180), usado no primeiro uso e em Configurações ›
 * Projetos: 1. de onde vem o código (clonar ou pasta existente), 2. o que será lido
 * (prévia automática), 3. demandas (opcional), 4. revisar e cadastrar, 5. concluído,
 * que leva ao preparo do ambiente do Claude.
 */
export function AssistenteProjeto({
  api,
  aoConcluir,
  aoPrepararAmbiente,
  aoCancelar,
}: {
  api: ApiAssistenteProjeto;
  /** “Fazer depois” no fim (e “Preparar o ambiente”, se não houver {@link aoPrepararAmbiente}). */
  aoConcluir: (projeto: ProjetoCadastrado) => void;
  aoPrepararAmbiente?: (projeto: ProjetoCadastrado) => void;
  aoCancelar?: () => void;
}) {
  const [passo, setPasso] = useState<Passo>(1);
  const [codigo, setCodigo] = useState<EstadoCodigo>(codigoInicial);
  const [clonando, setClonando] = useState(false);
  const [exclusoes, setExclusoes] = useState("");
  const [inclusoes, setInclusoes] = useState<string[]>([]);
  const [previa, setPrevia] = useState<{ chave: string; dados: PreviaProjeto } | null>(null);
  const [falhaPrevia, setFalhaPrevia] = useState<{ chave: string; texto: string } | null>(null);
  const [tentativaPrevia, setTentativaPrevia] = useState(0);
  const [demandas, setDemandas] = useState<EstadoDemandas>(DEMANDAS_INICIAIS);
  const [cadastrando, setCadastrando] = useState(false);
  const [cadastrado, setCadastrado] = useState<ProjetoCadastrado | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  /** Veio do Revisar por um “Editar”: dá para voltar direto a ele. */
  const [vindoDoRevisar, setVindoDoRevisar] = useState(false);
  const [cancelando, setCancelando] = useState(false);
  const [avisoClone, setAvisoClone] = useState<string | null>(null);
  /** O último nome conferido na base e se já está em uso. */
  const [nomeConferido, setNomeConferido] = useState<{ nome: string; emUso: boolean } | null>(null);
  const titulo = useRef<HTMLHeadingElement>(null);
  const montado = useRef(false);
  const cancelouClone = useRef(false);

  const repositoriosCodigo = codigoDoProjeto(codigo);
  const pedido: PedidoProjeto = {
    nome: nomeDoProjeto(codigo),
    pastas: pastasDoProjeto(codigo),
    exclusoes: linhas(exclusoes),
    inclusoes,
    repositorios: { codigo: repositoriosCodigo, tracker: trackerDoProjeto(demandas, repositoriosCodigo) },
  };
  const chave = chaveDoPedido(pedido);
  const previaAtual = previa !== null && previa.chave === chave;
  const falhaAtual = falhaPrevia !== null && falhaPrevia.chave === chave ? falhaPrevia.texto : null;
  // No Revisar, depois de um “Editar”, a prévia pode estar desatualizada e é refeita.
  const precisaPrevia = passo === 2 || passo === 4;
  const lendo = precisaPrevia && !previaAtual && falhaAtual === null;
  const nomeAtual = pedido.nome;
  const nomeEmUso = nomeConferido?.nome === nomeAtual && nomeConferido.emUso;

  // O foco vai para o título do passo novo (não ao abrir o assistente).
  useEffect(() => {
    if (!montado.current) {
      montado.current = true;
      return;
    }
    titulo.current?.focus();
  }, [passo]);

  // Nome já em uso: confere no passo 1 (com a digitação assentada), antes da prévia.
  useEffect(() => {
    if (passo !== 1 || !nomeAtual) return;
    let ativo = true;
    const t = setTimeout(() => {
      Promise.resolve()
        .then(() => api.nomeEmUso(nomeAtual))
        .then((emUso) => ativo && setNomeConferido({ nome: nomeAtual, emUso }))
        .catch(() => undefined);
    }, 300);
    return () => {
      ativo = false;
      clearTimeout(t);
    };
  }, [api, passo, nomeAtual]);

  // Prévia automática ao entrar no passo 2 (ou no Revisar) e sempre que o pedido muda.
  useEffect(() => {
    if (!precisaPrevia || previaAtual || falhaAtual !== null) return;
    let ativo = true;
    const p = pedido;
    const t = setTimeout(
      () => {
        api
          .previaProjeto(p)
          .then((dados) => {
            if (!ativo) return;
            setPrevia({ chave, dados });
            setFalhaPrevia(null);
          })
          .catch((e: unknown) => ativo && setFalhaPrevia({ chave, texto: mensagemDe(e) }));
      },
      previa ? 400 : 0,
    );
    return () => {
      ativo = false;
      clearTimeout(t);
    };
    // O pedido muda junto com a chave.
  }, [api, precisaPrevia, chave, tentativaPrevia]);

  const irPara = (n: Passo) => {
    setFalha(null);
    setAvisoClone(null);
    if (n === 4) setVindoDoRevisar(false);
    setPasso(n);
  };

  /** “Editar” no Revisar: leva ao passo e deixa um atalho para voltar. */
  const editarDoRevisar = (n: Passo) => {
    irPara(n);
    setVindoDoRevisar(true);
  };

  // ---- Passo 1: validação e clone ----

  const validarCodigo = (): string | null => {
    if (codigo.origem === "pasta") {
      if (!codigo.pasta) return "Escolha a pasta do código do projeto.";
      if (codigo.detectados === null) return "Espere a busca dos repositórios git na pasta terminar.";
    } else {
      const lista = clonesPreenchidos(codigo);
      if (lista.length === 0) return "Informe a URL de pelo menos um repositório.";
      const i = codigo.clones.findIndex((c) => c.repo.url_clone.trim() && !c.repo.caminho);
      if (i >= 0) {
        return `Repositório ${i + 1}: não reconheci a URL de clone. Use HTTPS ou SSH, como https://github.com/dono/repo.git.`;
      }
      if (!codigo.pastaBase.trim()) return "Informe a pasta onde guardar os clones.";
    }
    const invalido = validarRepositorios({ codigo: codigoDoProjeto(codigo), tracker: null });
    if (invalido) return invalido;
    if (!nomeDoProjeto(codigo)) return "Dê um nome ao projeto.";
    return null;
  };

  /** Clona os repositórios e, se todos ficarem prontos, vai a `destino` (se houver). */
  const clonar = async (chaves: number[], destino: Passo | null) => {
    const alvos = codigo.clones.filter((c) => chaves.includes(c.chave));
    if (alvos.length === 0) return;
    cancelouClone.current = false;
    setCancelando(false);
    setAvisoClone(null);
    setClonando(true);
    setFalha(null);
    try {
      for (const c of alvos) {
        if (c.repo.token === "proprio" && c.token.trim()) {
          await api.salvarToken(alvoDoToken(c.repo), c.token.trim());
        }
      }
    } catch (e) {
      setFalha(`Não deu para guardar o token no chaveiro: ${mensagemDe(e)}`);
      setClonando(false);
      return;
    }
    if (cancelouClone.current) {
      // Cancelou enquanto o token ia para o chaveiro: nada chegou a ser clonado.
      setAvisoClone("Clone cancelado.");
      setClonando(false);
      setCancelando(false);
      return;
    }
    const ordem = alvos.map((c) => c.chave);
    setCodigo((e) => ({
      ...e,
      clones: e.clones.map((c) =>
        ordem.includes(c.chave)
          ? { ...c, token: c.repo.token === "proprio" ? "" : c.token, progresso: { ...PROGRESSO_CLONE_INICIAL } }
          : c,
      ),
    }));
    const aoEvento = (ev: EventoClone) => {
      const alvo = ordem[ev.indice];
      if (alvo === undefined) return;
      setCodigo((e) => ({
        ...e,
        clones: e.clones.map((c) =>
          c.chave === alvo
            ? {
                ...c,
                progresso: reduzirProgressoClone([c.progresso ?? PROGRESSO_CLONE_INICIAL], {
                  ...ev,
                  indice: 0,
                })[0]!,
              }
            : c,
        ),
      }));
    };
    try {
      const resultados = await api.clonarRepositorios(
        alvos.map((c) => c.repo),
        codigo.pastaBase.trim(),
        aoEvento,
      );
      const outrosProntos = clonesPreenchidos(codigo).every(
        (c) => ordem.includes(c.chave) || c.repo.pasta !== null,
      );
      const cancelados = resultados.some((r) => r.cancelado);
      if (cancelados) {
        setAvisoClone("Clone cancelado. A pasta que ele tinha começado foi apagada.");
      }
      const tudoOk =
        !cancelados &&
        outrosProntos &&
        resultados.length === ordem.length &&
        resultados.every((r) => !r.erro && r.repositorio.pasta);
      setCodigo((e) => ({
        ...e,
        editandoClones: tudoOk ? false : e.editandoClones,
        clones: e.clones.map((c) => {
          const j = ordem.indexOf(c.chave);
          const r = j >= 0 ? resultados[j] : undefined;
          if (!r) return c;
          if (r.cancelado) return { ...c, progresso: null };
          if (r.erro || !r.repositorio.pasta) {
            return {
              ...c,
              progresso: {
                ...(c.progresso ?? PROGRESSO_CLONE_INICIAL),
                situacao: "falhou",
                erro: r.erro ?? "o clone não informou a pasta",
              },
            };
          }
          return {
            ...c,
            repo: { ...c.repo, pasta: r.repositorio.pasta },
            progresso: {
              ...(c.progresso ?? PROGRESSO_CLONE_INICIAL),
              situacao: "concluido",
              percentual: 100,
              pasta: r.repositorio.pasta,
            },
          };
        }),
      }));
      if (tudoOk && destino !== null) irPara(destino);
    } catch (e) {
      setFalha(mensagemDe(e));
      setCodigo((est) => ({
        ...est,
        clones: est.clones.map((c) =>
          ordem.includes(c.chave) && c.progresso?.situacao !== "concluido"
            ? { ...c, progresso: null }
            : c,
        ),
      }));
    } finally {
      setClonando(false);
      setCancelando(false);
    }
  };

  const cancelarClone = () => {
    cancelouClone.current = true;
    setCancelando(true);
    // O clone devolve os resultados (com `cancelado`) quando o git para.
    void Promise.resolve(api.cancelarClone()).catch(() => undefined);
  };

  /** Passo 1 → `destino` (o passo 2, ou o Revisar quando se veio de lá). */
  const avancarCodigo = (destino: Passo = 2) => {
    const invalido = validarCodigo();
    if (invalido) {
      setFalha(invalido);
      return;
    }
    const nome = nomeDoProjeto(codigo);
    setFalha(null);
    void Promise.resolve(api.nomeEmUso(nome))
      .catch(() => false)
      .then((emUso) => {
        setNomeConferido({ nome, emUso });
        if (emUso) {
          document.getElementById("nome-projeto")?.focus();
          return;
        }
        if (codigo.origem === "clonar" && !todosClonados(codigo)) {
          void clonar(
            clonesPreenchidos(codigo)
              .filter((c) => c.repo.pasta === null)
              .map((c) => c.chave),
            destino,
          );
          return;
        }
        irPara(destino);
      });
  };

  // ---- Passo 2 ----

  const avancarLeitura = (destino: Passo = 3) => {
    if (!previaAtual) {
      setFalha(falhaAtual ?? "Espere a prévia terminar.");
      return;
    }
    if (previa.dados.jaCadastrado) {
      setFalha(`Já existe um projeto cadastrado com o nome “${previa.dados.nome}”. Volte ao passo 1 e use outro nome.`);
      return;
    }
    irPara(destino);
  };

  const incluirPadroes = (padroes: string[]) =>
    setInclusoes((l) => [...l, ...padroes.filter((p) => !l.includes(p))]);

  const incluir = (caminho: string) => incluirPadroes([padraoDoCaminho(caminho)]);

  // ---- Passo 3 ----

  const validarDemandas = (): string | null => {
    const tracker = pedido.repositorios?.tracker ?? null;
    const invalido = validarRepositorios({ codigo: repositoriosCodigo, tracker });
    if (invalido) return invalido;
    if (
      tracker?.kind === "jira" &&
      !tracker.credential_cmd.trim() &&
      !demandas.tokenJira.trim() &&
      demandas.jiraGuardado !== true
    ) {
      return "Informe o token do Jira.";
    }
    return null;
  };

  const avancarDemandas = () => {
    const invalido = validarDemandas();
    if (invalido) {
      setFalha(invalido);
      return;
    }
    irPara(4);
  };

  /** “Voltar ao Revisar” depois de um “Editar”: confere o passo atual e o das demandas. */
  const voltarAoRevisar = () => {
    if (clonando || cadastrando) return;
    if (passo === 1) {
      avancarCodigo(4);
      return;
    }
    if (passo === 2) {
      if (!previaAtual) {
        setFalha(falhaAtual ?? "Espere a prévia terminar.");
        return;
      }
      if (previa.dados.jaCadastrado) {
        setFalha(`Já existe um projeto cadastrado com o nome “${previa.dados.nome}”. Volte ao passo 1 e use outro nome.`);
        return;
      }
    }
    // O que mudou antes pode ter deixado as demandas sem sentido (um repositório que saiu).
    const invalido = validarDemandas();
    if (invalido) {
      irPara(3);
      setFalha(invalido);
      return;
    }
    irPara(4);
  };

  // ---- Passo 4 ----

  const cadastrar = async () => {
    setCadastrando(true);
    setFalha(null);
    try {
      const tracker = pedido.repositorios?.tracker;
      if (tracker?.kind === "jira" && demandas.tokenJira.trim()) {
        await api.salvarToken({ host: hostDaUrl(tracker.url), caminho: null }, demandas.tokenJira.trim());
        setDemandas((d) => ({ ...d, tokenJira: "", jiraGuardado: true }));
      }
      const projeto = await api.cadastrarProjeto(pedido);
      setCadastrado(projeto);
      irPara(5);
    } catch (e) {
      setFalha(mensagemDe(e));
    } finally {
      setCadastrando(false);
    }
  };

  const avancar = (ev?: FormEvent) => {
    ev?.preventDefault();
    if (clonando || cadastrando) return;
    if (passo === 1) avancarCodigo();
    else if (passo === 2) avancarLeitura();
    else if (passo === 3) avancarDemandas();
    else if (passo === 4) void cadastrar();
  };

  if (passo === 5 && cadastrado) {
    return (
      <Concluido
        projeto={cadastrado}
        repositorios={repositoriosCodigo.length}
        tituloRef={titulo}
        aoConcluir={aoConcluir}
        aoPrepararAmbiente={aoPrepararAmbiente ?? aoConcluir}
      />
    );
  }

  const etapa = TITULOS[passo as Exclude<Passo, 5>];
  const textoPrimario =
    passo === 4
      ? cadastrando
        ? "Cadastrando…"
        : "Cadastrar o projeto"
      : passo === 1 && codigo.origem === "clonar" && !todosClonados(codigo)
        ? clonando
          ? "Clonando…"
          : "Clonar e continuar"
        : "Continuar";

  return (
    <form className="assistente-projeto" aria-label="Cadastrar projeto" onSubmit={avancar} noValidate>
      <nav aria-label="Etapas do cadastro">
        <ol className="etapas-cadastro">
          {ETAPAS.map((e) => {
            const feita = e.n < passo;
            const atual = e.n === passo;
            return (
              <li key={e.n} className={atual ? "atual" : feita ? "feita" : "futura"}>
                <button
                  type="button"
                  aria-current={atual ? "step" : undefined}
                  disabled={!feita && !atual}
                  onClick={() => feita && !clonando && !cadastrando && irPara(e.n)}
                >
                  <span className="circulo" aria-hidden="true">
                    {feita ? "✓" : e.n}
                  </span>
                  <span className="textos">
                    <span className="rotulo">
                      <span className="sr-only">Etapa {e.n}: </span>
                      {e.rotulo}
                      {feita && <span className="sr-only"> (feita)</span>}
                    </span>
                    <span className="sub">{e.sub}</span>
                  </span>
                </button>
              </li>
            );
          })}
        </ol>
        <div className="trilho-etapas" aria-hidden="true">
          <div className="barra" style={{ width: `${(passo / 4) * 100}%` }} />
        </div>
      </nav>

      <section className="passo-cadastro" aria-labelledby="titulo-passo-cadastro">
        <div className="cabecalho-passo">
          <h2 id="titulo-passo-cadastro" ref={titulo} tabIndex={-1}>
            {etapa.titulo}
          </h2>
          <p className="muted">{etapa.texto}</p>
        </div>

        {passo === 1 && (
          <PassoCodigo
            api={api}
            codigo={codigo}
            mudar={setCodigo}
            clonando={clonando}
            cancelando={cancelando}
            nomeEmUso={nomeEmUso}
            aviso={avisoClone}
            aoTentarDeNovo={(c) => void clonar([c], null)}
            aoCancelarClone={cancelarClone}
          />
        )}
        {passo === 2 && (
          <PassoLeitura
            previa={previa?.dados ?? null}
            lendo={lendo}
            falha={falhaAtual}
            exclusoes={exclusoes}
            inclusoes={inclusoes}
            aoMudarExclusoes={setExclusoes}
            aoIncluir={incluir}
            aoIncluirGrupo={incluirPadroes}
            aoTirarInclusao={(x) => setInclusoes((l) => l.filter((y) => y !== x))}
            aoTentarDeNovo={() => {
              setFalhaPrevia(null);
              setTentativaPrevia((t) => t + 1);
            }}
          />
        )}
        {passo === 3 && (
          <PassoDemandas
            api={api}
            codigo={repositoriosCodigo}
            demandas={demandas}
            mudar={setDemandas}
          />
        )}
        {passo === 4 && (
          <Revisao
            api={api}
            codigo={codigo}
            pedido={pedido}
            previa={previaAtual ? previa.dados : null}
            lendo={lendo}
            falhaPrevia={falhaAtual}
            demandas={demandas}
            aoEditar={editarDoRevisar}
            aoTentarDeNovo={() => {
              setFalhaPrevia(null);
              setTentativaPrevia((t) => t + 1);
            }}
          />
        )}

        {cadastrando && (
          <p role="status" className="progresso">
            <progress aria-label="Inventário do projeto" /> Cadastrando e inventariando as pastas do
            projeto…
          </p>
        )}
        {falha && (
          <p className="error-text" role="alert">
            {falha}
          </p>
        )}
      </section>

      <div className="rodape-assistente">
        {aoCancelar && (
          <button type="button" className="link" onClick={aoCancelar} disabled={clonando || cadastrando}>
            Cancelar
          </button>
        )}
        <span className="espaco" />
        {passo > 1 && (
          <button
            type="button"
            className="btn"
            disabled={clonando || cadastrando}
            onClick={() => irPara((passo - 1) as Passo)}
          >
            Voltar
          </button>
        )}
        {vindoDoRevisar && passo < 4 && (
          <button
            type="button"
            className="btn"
            disabled={clonando || cadastrando || (passo === 2 && lendo)}
            onClick={voltarAoRevisar}
          >
            Voltar ao Revisar
          </button>
        )}
        {passo === 3 && (
          <button
            type="button"
            className="btn"
            onClick={() => {
              setDemandas((d) => ({ ...d, tipo: "nenhum" }));
              irPara(4);
            }}
          >
            Pular esta etapa
          </button>
        )}
        <button
          type="submit"
          className="btn btn-primary"
          disabled={
            clonando ||
            cadastrando ||
            (passo === 2 && lendo) ||
            (passo === 4 && (!previaAtual || previa.dados.jaCadastrado))
          }
        >
          {textoPrimario}
        </button>
      </div>
    </form>
  );
}

/** O token de um repositório: o global só aparece como tal se há token guardado para o host. */
function textoToken(
  r: Pick<RepositorioCodigo, "token" | "forja" | "host">,
  guardados: Record<string, boolean | null | undefined>,
): string {
  if (r.token === "proprio") return "próprio";
  if (r.token === "comando") return "por comando";
  const g = guardados[r.host.trim().toLowerCase()];
  if (g === false) return "sem token";
  if (g === undefined) return "verificando…";
  return `global do ${ROTULO_FORJA[r.forja]}`;
}

function textoDemandas(d: EstadoDemandas, pedido: PedidoProjeto): string {
  const t = pedido.repositorios?.tracker ?? null;
  if (!t || d.tipo === "nenhum") return "Nenhuma (demandas criadas no app)";
  if (t.kind === "github") return `GitHub Issues · ${t.repo}`;
  if (t.kind === "gitlab") return `GitLab Issues · ${t.path}`;
  return `Jira · projeto ${t.project_key}${t.active_sprint_only ? ", sprint ativa" : ""}`;
}

function Revisao({
  api,
  codigo,
  pedido,
  previa,
  lendo,
  falhaPrevia,
  demandas,
  aoEditar,
  aoTentarDeNovo,
}: {
  api: Pick<ApiAssistenteProjeto, "tokensGuardados">;
  codigo: EstadoCodigo;
  pedido: PedidoProjeto;
  /** `null` enquanto a prévia do que mudou não ficou pronta. */
  previa: PreviaProjeto | null;
  lendo: boolean;
  falhaPrevia: string | null;
  demandas: EstadoDemandas;
  aoEditar: (passo: Passo) => void;
  aoTentarDeNovo: () => void;
}) {
  const repos = pedido.repositorios?.codigo ?? [];
  const inv = previa?.inventario;
  const guardados = useTokensGuardados(
    api,
    hostsDistintos(repos.filter((r) => r.token === "global")),
  );
  const textoCodigo =
    codigo.origem === "clonar"
      ? `${plural(repos.length, "repositório clonado", "repositórios clonados")} em ${codigo.pastaBase}`
      : repos.length > 0
        ? `Pasta existente com ${plural(repos.length, "repositório git", "repositórios git")}: ${repos
            .map(rotuloRepositorio)
            .join(", ")}`
        : "Pasta existente";
  const linhasResumo: { rotulo: string; valor: ReactNode; passo: Passo }[] = [
    { rotulo: "Código", valor: textoCodigo, passo: 1 },
    {
      rotulo: pedido.pastas.length === 1 ? "Pasta" : "Pastas",
      valor: (
        <ul className="lista-simples">
          {pedido.pastas.map((p) => (
            <li key={p}>
              <code>{p}</code>
            </li>
          ))}
        </ul>
      ),
      passo: 1,
    },
    { rotulo: "Nome", valor: pedido.nome, passo: 1 },
    {
      rotulo: "O que será lido",
      valor: inv
        ? `${plural(inv.analisados, "arquivo", "arquivos")} (${tamanhoLegivel(inv.bytesAnalisados)}) · ${plural(inv.ignorados, "arquivo", "arquivos")} fora`
        : "lendo as pastas…",
      passo: 2,
    },
    { rotulo: "Demandas", valor: textoDemandas(demandas, pedido), passo: 3 },
  ];
  if (codigo.origem === "clonar" && repos.length > 0) {
    linhasResumo.splice(1, 0, {
      rotulo: "Tokens",
      valor: repos.map((r) => `${nomeDaPasta(r.caminho)}: ${textoToken(r, guardados)}`).join(" · "),
      passo: 1,
    });
  }
  return (
    <>
      <dl className="cartao-assistente resumo-cadastro">
        {linhasResumo.map((l) => (
          <div key={l.rotulo} className="linha-resumo">
            <dt>{l.rotulo}</dt>
            <dd>{l.valor}</dd>
            <button
              type="button"
              className="link"
              aria-label={`Editar ${l.rotulo.toLowerCase()}`}
              onClick={() => aoEditar(l.passo)}
            >
              Editar
            </button>
          </div>
        ))}
      </dl>
      {lendo && (
        <p role="status" className="progresso">
          <progress aria-label="Lendo as pastas do projeto" /> Lendo as pastas do projeto…
        </p>
      )}
      {falhaPrevia && !lendo && (
        <div className="falha-previa">
          <p className="error-text" role="alert">
            {falhaPrevia}
          </p>
          <button type="button" className="btn btn-sm" onClick={aoTentarDeNovo}>
            Tentar de novo
          </button>
        </div>
      )}
      {previa?.jaCadastrado && (
        <p className="error-text" role="alert">
          Já existe um projeto cadastrado com o nome “{previa.nome}”. Edite o nome no passo 1.
        </p>
      )}
      <p className="dica nota-seguranca">
        O app só lê as pastas do código. Os tokens ficam no chaveiro do sistema.
      </p>
    </>
  );
}

function Concluido({
  projeto,
  repositorios,
  tituloRef,
  aoConcluir,
  aoPrepararAmbiente,
}: {
  projeto: ProjetoCadastrado;
  repositorios: number;
  tituloRef: RefObject<HTMLHeadingElement>;
  aoConcluir: (projeto: ProjetoCadastrado) => void;
  aoPrepararAmbiente: (projeto: ProjetoCadastrado) => void;
}) {
  const onde =
    repositorios > 0
      ? plural(repositorios, "repositório", "repositórios")
      : plural(Math.max(1, projeto.pastas.length), "pasta", "pastas");
  return (
    <section className="assistente-projeto concluido" aria-labelledby="titulo-cadastro-concluido">
      <span className="selo-concluido ok-text" aria-hidden="true">
        ✓
      </span>
      <h2 id="titulo-cadastro-concluido" ref={tituloRef} tabIndex={-1}>
        “{projeto.nome}” está cadastrado
      </h2>
      <p>
        {plural(projeto.inventario.analisados, "arquivo inventariado", "arquivos inventariados")} em{" "}
        {onde}. Falta um passo: deixar o Claude conversar com o app.
      </p>
      <div className="acoes">
        <button type="button" className="btn" onClick={() => aoConcluir(projeto)}>
          Fazer depois
        </button>
        <button type="button" className="btn btn-primary" onClick={() => aoPrepararAmbiente(projeto)}>
          Preparar o ambiente do Claude
        </button>
      </div>
    </section>
  );
}

