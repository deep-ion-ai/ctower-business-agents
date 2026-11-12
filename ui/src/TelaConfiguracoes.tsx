import { useCallback, useEffect, useState } from "react";
import { mensagemDe, type SituacaoProtecao } from "./analise";
import { CadastroProjeto, ResumoDoInventario } from "./CadastroProjeto";
import {
  ESTADO_DO_TEXTO,
  apiConfiguracoesTauri,
  type AmbienteDetalhado,
  type ApiConfiguracoes,
  type AvaliacaoTroca,
  type Configuracoes,
  type GrupoTexto,
  type ModoTroca,
  type ProjetoConfig,
  type TextoEditavel,
  type TrocaBase,
} from "./configuracoes";
import { EscolhaBase } from "./EscolhaBase";
import { EscolhaCliente } from "./EscolhaCliente";
import { AmbienteDoProjeto } from "./PrepararAmbiente";
import { padraoDoCaminho, tamanhoLegivel, type PreviaProjeto } from "./primeiroUso";
import "./configuracoes.css";
import { THEME_PREFS, useTheme } from "./shell/theme";

const CLIENTE = {
  terminalEmbutido: "Claude Code no terminal do app",
  claudeDesktop: "Claude Desktop",
} as const;

/**
 * Configurações (spec, seção 10): cliente de IA preferido, pasta da base, projetos
 * (pastas, exclusões, ambiente do Claude, proteção contra edição, regras do projeto,
 * descadastro) e os textos editáveis da base (templates, roteiros e prompts). Monta
 * os passos do primeiro uso ({@link EscolhaCliente}, {@link EscolhaBase},
 * {@link CadastroProjeto}, {@link AmbienteDoProjeto}) com o que só as configurações
 * fazem: mover ou reapontar a base, desfazer o ambiente e descadastrar.
 */
export function TelaConfiguracoes({
  api = apiConfiguracoesTauri,
  aoMudarProjetos,
}: {
  api?: ApiConfiguracoes;
  /** Os projetos ou a base mudaram: as outras telas devem reler. */
  aoMudarProjetos?: () => void;
}) {
  const [cfg, setCfg] = useState<Configuracoes | null>(null);
  const [textos, setTextos] = useState<TextoEditavel[]>([]);
  const [falha, setFalha] = useState<string | null>(null);

  const carregar = useCallback(() => {
    api
      .configuracoes()
      .then((c) => {
        setCfg(c);
        setFalha(null);
        if (c.base) return api.textos().then(setTextos);
        setTextos([]);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  }, [api]);

  useEffect(carregar, [carregar]);

  const mudou = () => {
    carregar();
    aoMudarProjetos?.();
  };

  if (!cfg) {
    return (
      <section className="configuracoes page" aria-label="Configurações">
        {falha ? (
          <p className="error-text" role="alert">
            Não foi possível ler as configurações: {falha}
          </p>
        ) : (
          <p className="muted">Carregando as configurações…</p>
        )}
      </section>
    );
  }

  return (
    <section className="configuracoes page" aria-label="Configurações">
      <h2>Configurações</h2>
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      <SecaoAparencia />
      <SecaoCliente api={api} cfg={cfg} aoMudar={carregar} />
      <SecaoBase api={api} cfg={cfg} aoMudar={mudou} />
      <SecaoProjetos api={api} cfg={cfg} textos={textos} aoMudar={mudou} aoAbrirTexto={carregar} />
      {cfg.base && <SecaoTextos api={api} textos={textos} aoAbrir={carregar} />}
    </section>
  );
}

// ---------------------------------------------------------------------------
// Aparência (#85): tema claro, escuro ou do projeto, o mesmo da barra de título.

function SecaoAparencia() {
  const [pref, mudar] = useTheme();
  return (
    <section className="secao-config" aria-label="Aparência">
      <h3>Aparência</h3>
      <fieldset className="opcoes-troca">
        <legend className="sr-only">Tema</legend>
        {THEME_PREFS.map((t) => (
          <label key={t.pref} className={pref === t.pref ? "opcao escolhida" : "opcao"}>
            <input
              type="radio"
              name="tema"
              value={t.pref}
              checked={pref === t.pref}
              onChange={() => mudar(t.pref)}
            />
            <span>{t.rotulo}</span>
          </label>
        ))}
      </fieldset>
      <p className="muted">
        “Do sistema” acompanha o tema claro ou escuro do sistema operacional.
      </p>
    </section>
  );
}

// ---------------------------------------------------------------------------
// Cliente de IA

function SecaoCliente({
  api,
  cfg,
  aoMudar,
}: {
  api: ApiConfiguracoes;
  cfg: Configuracoes;
  aoMudar: () => void;
}) {
  const [trocando, setTrocando] = useState(false);
  return (
    <section className="secao-config" aria-label="Cliente de IA">
      <h3>Cliente de IA preferido</h3>
      <p>
        {CLIENTE[cfg.cliente ?? "terminalEmbutido"]}
        {!cfg.cliente && <span className="muted"> (padrão)</span>}
      </p>
      {!trocando ? (
        <button className="btn" type="button" onClick={() => setTrocando(true)}>
          Trocar o cliente
        </button>
      ) : (
        <>
          <EscolhaCliente
            api={api}
            inicial={cfg.cliente}
            rotuloContinuar="Salvar"
            aoConcluir={() => {
              setTrocando(false);
              aoMudar();
            }}
          />
          <button type="button" className="link" onClick={() => setTrocando(false)}>
            Cancelar
          </button>
        </>
      )}
    </section>
  );
}

// ---------------------------------------------------------------------------
// Pasta da base

function SecaoBase({
  api,
  cfg,
  aoMudar,
}: {
  api: ApiConfiguracoes;
  cfg: Configuracoes;
  aoMudar: () => void;
}) {
  const [etapa, setEtapa] = useState<"ver" | "escolher" | "confirmar">("ver");
  const [destino, setDestino] = useState<string | null>(null);
  const [feito, setFeito] = useState<TrocaBase | null>(null);

  // A escolha da pasta é a do primeiro uso, com as mesmas recusas; aqui ela só
  // escolhe: a troca de fato vem depois, com a confirmação.
  const apiEscolha = {
    avaliarBase: api.avaliarBase,
    escolherPasta: api.escolherPasta,
    configurarBase: (pasta: string) =>
      api.avaliarBase(pasta).then((a) => {
        if (a.recusa) throw new Error(a.recusa);
        return a.caminho;
      }),
  };

  return (
    <section className="secao-config" aria-label="Pasta da base">
      <h3>Pasta da base de conhecimento</h3>
      <p>
        {cfg.base ? <code>{cfg.base}</code> : <span className="muted">Ainda não escolhida.</span>}
      </p>
      {feito && <ResultadoTroca feito={feito} />}
      {etapa === "ver" && (
        <button
          className="btn"
          type="button"
          onClick={() => {
            setFeito(null);
            setEtapa("escolher");
          }}
        >
          Trocar a pasta da base
        </button>
      )}
      {etapa === "escolher" && (
        <>
          <EscolhaBase
            api={apiEscolha}
            atual={null}
            sugerida={cfg.base ? null : cfg.baseSugerida}
            rotuloContinuar="Continuar"
            aoConcluir={(p) => {
              setDestino(p);
              setEtapa("confirmar");
            }}
          />
          <button type="button" className="link" onClick={() => setEtapa("ver")}>
            Cancelar
          </button>
        </>
      )}
      {etapa === "confirmar" && destino && (
        <ConfirmarTroca
          api={api}
          destino={destino}
          aoCancelar={() => setEtapa("ver")}
          aoConcluir={(t) => {
            setFeito(t);
            setEtapa("ver");
            aoMudar();
          }}
        />
      )}
    </section>
  );
}

function ConfirmarTroca({
  api,
  destino,
  aoCancelar,
  aoConcluir,
}: {
  api: ApiConfiguracoes;
  destino: string;
  aoCancelar: () => void;
  aoConcluir: (t: TrocaBase) => void;
}) {
  const [avaliacao, setAvaliacao] = useState<AvaliacaoTroca | null>(null);
  const [modo, setModo] = useState<ModoTroca | null>(null);
  const [confirmado, setConfirmado] = useState(false);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  useEffect(() => {
    let ativo = true;
    api
      .avaliarTroca(destino)
      .then((a) => {
        if (!ativo) return;
        setAvaliacao(a);
        setModo(!a.recusaMover ? "mover" : !a.recusaReapontar ? "reapontar" : null);
      })
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, destino]);

  const trocar = () => {
    if (!modo) return;
    setOcupado(true);
    setFalha(null);
    api
      .trocarBase(destino, modo)
      .then(aoConcluir)
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const recusa = (m: ModoTroca) =>
    m === "mover" ? avaliacao?.recusaMover : avaliacao?.recusaReapontar;

  return (
    <div className="confirmar-troca" role="region" aria-label="Confirmar a troca da base">
      <p>
        Pasta nova: <code>{destino}</code>
      </p>
      {!avaliacao && !falha && <p className="muted">Conferindo a pasta…</p>}
      {avaliacao && (
        <>
          {avaliacao.atual && (
            <p className="muted">
              A base atual ({avaliacao.atual}) tem {avaliacao.arquivos} arquivos (
              {tamanhoLegivel(avaliacao.bytes)}).
            </p>
          )}
          <fieldset className="opcoes-troca">
            <legend>O que fazer com os documentos?</legend>
            <label className={modo === "mover" ? "opcao escolhida" : "opcao"}>
              <input
                type="radio"
                name="modo-troca"
                value="mover"
                checked={modo === "mover"}
                disabled={!!avaliacao.recusaMover}
                onChange={() => {
                  setModo("mover");
                  setConfirmado(false);
                }}
              />
              <strong>Mover os documentos para a pasta nova</strong>
              <span className="muted">
                O app copia a base inteira, confere cada arquivo e só então passa a usar a pasta
                nova e apaga a antiga. Se algo falhar, nada muda.
              </span>
              {avaliacao.recusaMover && <span className="attention-text">{avaliacao.recusaMover}</span>}
            </label>
            <label className={modo === "reapontar" ? "opcao escolhida" : "opcao"}>
              <input
                type="radio"
                name="modo-troca"
                value="reapontar"
                checked={modo === "reapontar"}
                disabled={!!avaliacao.recusaReapontar}
                onChange={() => {
                  setModo("reapontar");
                  setConfirmado(false);
                }}
              />
              <strong>Só passar a usar a pasta nova</strong>
              <span className="muted">
                Os documentos da pasta atual ficam onde estão, intactos, mas deixam de aparecer no
                app. Útil quando a pasta nova já é uma base (outra máquina, outro clone).
              </span>
              {avaliacao.recusaReapontar && (
                <span className="attention-text">{avaliacao.recusaReapontar}</span>
              )}
            </label>
          </fieldset>
          {avaliacao.sessoesAbertas > 0 && (
            <p className="attention-text" role="status">
              Há {avaliacao.sessoesAbertas} conversa(s) com o Claude que podem estar abertas. Feche
              as conversas antes de trocar a base, para nada ser gravado na pasta antiga.
            </p>
          )}
          {modo && (
            <label>
              <input
                type="checkbox"
                checked={confirmado}
                onChange={(e) => setConfirmado(e.target.checked)}
              />{" "}
              {modo === "mover"
                ? "Confirmo: mover os documentos e apagar a pasta antiga depois da conferência"
                : "Confirmo: passar a usar a pasta nova, deixando os documentos atuais onde estão"}
            </label>
          )}
          {!modo && (
            <p className="error-text" role="alert">
              {recusa("reapontar") ?? recusa("mover")}
            </p>
          )}
        </>
      )}
      {ocupado && (
        <div role="status" className="progresso">
          <progress aria-label="Troca da base" />{" "}
          {modo === "mover" ? "Copiando e conferindo os documentos…" : "Trocando a base…"}
        </div>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      <div className="acoes">
        <button className="btn" type="button" onClick={aoCancelar} disabled={ocupado}>
          Cancelar
        </button>
        <button
          type="button"
          className="btn btn-primary"
          onClick={trocar}
          disabled={!modo || !confirmado || ocupado}
        >
          {modo === "reapontar" ? "Usar a pasta nova" : "Mover a base"}
        </button>
      </div>
    </div>
  );
}

function ResultadoTroca({ feito }: { feito: TrocaBase }) {
  return (
    <div className="ok-text" role="status">
      <p>
        {feito.modo === "mover"
          ? `Base movida: ${feito.copiados} arquivos copiados e conferidos${
              feito.origemApagada ? "; a pasta antiga foi apagada" : ""
            }.`
          : "O app passou a usar a pasta nova."}{" "}
        {feito.indexados !== null && `Índice reconstruído com ${feito.indexados} documentos.`}
      </p>
      {feito.avisos.map((a) => (
        <p key={a} className="attention-text">
          {a}
        </p>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Projetos

function SecaoProjetos({
  api,
  cfg,
  textos,
  aoMudar,
  aoAbrirTexto,
}: {
  api: ApiConfiguracoes;
  cfg: Configuracoes;
  textos: TextoEditavel[];
  aoMudar: () => void;
  aoAbrirTexto: () => void;
}) {
  const [cadastrando, setCadastrando] = useState(false);
  return (
    <section className="secao-config" aria-label="Projetos">
      <h3>Projetos</h3>
      {cfg.projetos.length === 0 && <p className="muted">Nenhum projeto cadastrado.</p>}
      {cfg.configClaude && (
        <p className="muted">
          O ambiente do Claude fica em {cfg.configClaude.estado} (servidor MCP) e{" "}
          {cfg.configClaude.settings} (hooks e proteção).
        </p>
      )}
      {cfg.projetos.map((s) => (
        <CartaoProjeto
          key={s.id}
          api={api}
          projeto={s}
          regras={textos.find((t) => t.grupo === "regrasDoProjeto" && t.id === s.id)}
          aoMudar={aoMudar}
          aoAbrirTexto={aoAbrirTexto}
        />
      ))}
      {cfg.base &&
        (cadastrando ? (
          <>
            <CadastroProjeto
              api={api}
              aoConcluir={() => {
                setCadastrando(false);
                aoMudar();
              }}
            />
            <button type="button" className="link" onClick={() => setCadastrando(false)}>
              Cancelar
            </button>
          </>
        ) : (
          <button className="btn" type="button" onClick={() => setCadastrando(true)}>
            Cadastrar outro projeto
          </button>
        ))}
    </section>
  );
}

function CartaoProjeto({
  api,
  projeto,
  regras,
  aoMudar,
  aoAbrirTexto,
}: {
  api: ApiConfiguracoes;
  projeto: ProjetoConfig;
  regras?: TextoEditavel;
  aoMudar: () => void;
  aoAbrirTexto: () => void;
}) {
  // Muda depois de desfazer o ambiente, para a verificação rodar de novo.
  const [vez, setVez] = useState(0);
  return (
    <article className="cartao-projeto" aria-label={projeto.nome}>
      <h4>{projeto.nome}</h4>
      <ul className="muted">
        {projeto.pastas.map((p) => (
          <li key={p.caminho}>
            {p.caminho}
            {p.prefixo && ` (aparece como ${p.prefixo}/)`}
            {!p.existe && <strong className="attention-text"> · a pasta não existe mais</strong>}
          </li>
        ))}
      </ul>
      <p className="muted">
        Para mudar as pastas, descadastre o projeto e cadastre de novo (os documentos ficam na
        base).
      </p>
      <AjusteDeExclusoes api={api} projeto={projeto} aoSalvar={aoMudar} />
      <div role="group" aria-label={`Ambiente do Claude de ${projeto.nome}`}>
        <h5>Ambiente do Claude</h5>
        <DetalheDoAmbiente key={`d${vez}`} api={api} projeto={projeto.id} />
        <AmbienteDoProjeto key={vez} api={api} projeto={projeto.id} />
        <DesfazerAmbiente api={api} projeto={projeto} aoDesfazer={() => setVez((v) => v + 1)} />
      </div>
      <Protecao key={`p${vez}`} api={api} projeto={projeto} />
      {regras && (
        <div role="group" aria-label={`Regras do projeto ${projeto.nome}`}>
          <h5>Regras do projeto</h5>
          <p className="muted">
            Instruções que o agente recebe em toda análise deste projeto ({regras.arquivo}):{" "}
            {ESTADO_DO_TEXTO[regras.estado]}.
          </p>
          <AbrirTexto api={api} texto={regras} aoAbrir={aoAbrirTexto} />
        </div>
      )}
      <Descadastrar api={api} projeto={projeto} aoDescadastrar={aoMudar} />
    </article>
  );
}

function linhas(texto: string): string[] {
  return texto
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
}

function AjusteDeExclusoes({
  api,
  projeto,
  aoSalvar,
}: {
  api: ApiConfiguracoes;
  projeto: ProjetoConfig;
  aoSalvar: () => void;
}) {
  const [exclusoes, setExclusoes] = useState(projeto.exclusoes.join("\n"));
  const [inclusoes, setInclusoes] = useState<string[]>(projeto.inclusoes);
  const [previa, setPrevia] = useState<PreviaProjeto | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [salvo, setSalvo] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  const ajuste = (inc = inclusoes) => ({
    projeto: projeto.id,
    exclusoes: linhas(exclusoes),
    inclusoes: inc,
  });

  const verPrevia = (inc = inclusoes) => {
    setOcupado(true);
    setFalha(null);
    setSalvo(false);
    api
      .previaExclusoes(ajuste(inc))
      .then(setPrevia)
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const salvar = () => {
    setOcupado(true);
    setFalha(null);
    api
      .salvarExclusoes(ajuste())
      .then((s) => {
        setPrevia(null);
        setSalvo(true);
        setFalha(null);
        aoSalvar();
        return s;
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  const mudarInclusoes = (novas: string[]) => {
    setInclusoes(novas);
    verPrevia(novas);
  };

  return (
    <details className="ajuste-exclusoes">
      <summary>Exclusões: o que fica fora da análise</summary>
      <label>
        Excluir também (um padrão por linha, como no .gitignore)
        <textarea
          rows={3}
          value={exclusoes}
          spellCheck={false}
          onChange={(e) => {
            setExclusoes(e.target.value);
            setSalvo(false);
          }}
        />
      </label>
      {inclusoes.length > 0 && (
        <div role="group" aria-label="Reincluídos">
          <p>Reincluídos por você:</p>
          <ul>
            {inclusoes.map((x) => (
              <li key={x}>
                <code>{x}</code>{" "}
                <button
                  className="btn"
                  type="button"
                  aria-label={`Tirar a reinclusão de ${x}`}
                  onClick={() => mudarInclusoes(inclusoes.filter((y) => y !== x))}
                >
                  Desfazer
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
      {previa && (
        <ResumoDoInventario
          inventario={previa.inventario}
          ocupado={ocupado}
          aoReincluir={(c) => {
            const p = padraoDoCaminho(c);
            if (!inclusoes.includes(p)) mudarInclusoes([...inclusoes, p]);
          }}
        />
      )}
      {salvo && (
        <p className="ok-text" role="status">
          Exclusões salvas e projeto inventariado de novo.
        </p>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
      <div className="acoes">
        <button className="btn" type="button" onClick={() => verPrevia()} disabled={ocupado}>
          Ver o que será analisado
        </button>
        <button type="button" className="btn btn-primary" onClick={salvar} disabled={ocupado}>
          Salvar as exclusões
        </button>
      </div>
    </details>
  );
}

const SITUACAO = {
  ok: "registrado",
  ausente: "ausente",
  desatualizado: "desatualizado",
  conflito: "conflito (há um servidor “cba” que não é do app)",
} as const;

function DetalheDoAmbiente({ api, projeto }: { api: ApiConfiguracoes; projeto: string }) {
  const [ambiente, setAmbiente] = useState<AmbienteDetalhado | null>(null);
  useEffect(() => {
    let ativo = true;
    api
      .verificarAmbiente(projeto)
      .then((a) => ativo && setAmbiente(a))
      .catch(() => ativo && setAmbiente(null));
    return () => {
      ativo = false;
    };
  }, [api, projeto]);
  const v = ambiente?.verificacao;
  if (!v) return null;
  const hooksOk = v.hooks.filter((h) => h.situacao === "ok").length;
  return (
    <ul className="detalhe-ambiente" aria-label="Situação do ambiente">
      <li>Servidor MCP “cba”: {SITUACAO[v.servidorMcp]}</li>
      <li>
        Hooks: {hooksOk} de {v.hooks.length} registrados
        {v.hooksDesativados && " (desativados nas configurações do Claude)"}
      </li>
    </ul>
  );
}

function DesfazerAmbiente({
  api,
  projeto,
  aoDesfazer,
}: {
  api: ApiConfiguracoes;
  projeto: ProjetoConfig;
  aoDesfazer: () => void;
}) {
  const [confirmando, setConfirmando] = useState(false);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const [feito, setFeito] = useState<string[] | null>(null);

  const desfazer = () => {
    setOcupado(true);
    setFalha(null);
    api
      .desfazerAmbiente(projeto.id)
      .then((r) => {
        setConfirmando(false);
        setFeito(r.avisos);
        aoDesfazer();
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  return (
    <div className="desfazer-ambiente">
      {feito && (
        <div className="ok-text" role="status">
          <p>
            Ambiente desfeito: a configuração do Claude voltou a ser a de antes para este projeto.
          </p>
          {feito.map((a) => (
            <p key={a} className="attention-text">
              {a}
            </p>
          ))}
        </div>
      )}
      {!confirmando ? (
        <button
          className="btn"
          type="button"
          onClick={() => {
            setFeito(null);
            setConfirmando(true);
          }}
        >
          Desfazer o ambiente
        </button>
      ) : (
        <div className="confirmar-alteracao" role="region" aria-label="Confirmar desfazer o ambiente">
          <p>
            O app vai tirar da configuração do Claude o servidor MCP deste projeto, a proteção
            contra edição (se ligada) e, se nenhum outro projeto usar, os hooks. Sem eles, as
            conversas nesta pasta não enxergam o ctower. Você pode preparar de novo depois.
          </p>
          <button type="button" className="btn btn-primary" onClick={desfazer} disabled={ocupado}>
            Desfazer
          </button>{" "}
          <button
            className="btn"
            type="button"
            onClick={() => setConfirmando(false)}
            disabled={ocupado}
          >
            Cancelar
          </button>
        </div>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </div>
  );
}

function Protecao({ api, projeto }: { api: ApiConfiguracoes; projeto: ProjetoConfig }) {
  const [situacao, setSituacao] = useState<SituacaoProtecao | null>(null);
  const [confirmando, setConfirmando] = useState(false);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  useEffect(() => {
    let ativo = true;
    api
      .situacaoProtecao(projeto.id)
      .then((s) => ativo && setSituacao(s))
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, projeto.id]);

  const mudar = (ligada: boolean) => {
    setOcupado(true);
    setFalha(null);
    api
      .mudarProtecao(projeto.id, ligada)
      .then((s) => {
        setSituacao(s);
        setConfirmando(false);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  return (
    <div role="group" aria-label={`Proteção contra edição de ${projeto.nome}`}>
      <h5>Proteção contra edição</h5>
      {situacao && (
        <p>
          {situacao.ligada
            ? situacao.completa
              ? "Ligada: o Claude não edita arquivos nem usa o terminal nas pastas do projeto."
              : "Ligada, mas faltam regras no arquivo de configuração do Claude. Ligue de novo para completar."
            : "Desligada (padrão): o app só avisa quando a pasta do projeto é alterada."}
        </p>
      )}
      {situacao && !confirmando && (
        <button className="btn" type="button" onClick={() => setConfirmando(true)}>
          {situacao.ligada ? "Desligar a proteção" : "Ligar a proteção"}
        </button>
      )}
      {situacao && confirmando && (
        <div className="confirmar-alteracao">
          {situacao.ligada ? (
            <p>
              O app vai tirar de {situacao.arquivo} só as regras que ele mesmo acrescentou, e o
              terminal (Bash) volta a funcionar nas conversas deste projeto.
            </p>
          ) : (
            <>
              <p>
                O app vai acrescentar em {situacao.arquivo} estas regras, que valem mesmo que você
                aprove a edição, e negar o terminal ({situacao.negadasPeloHook.join(", ")}) nas
                conversas deste projeto:
              </p>
              <ul>
                {situacao.regras.map((r) => (
                  <li key={r.regra}>
                    <code>{r.regra}</code>
                  </li>
                ))}
              </ul>
            </>
          )}
          <button
            type="button"
            className="btn btn-primary"
            onClick={() => mudar(!situacao.ligada)}
            disabled={ocupado}
          >
            {situacao.ligada ? "Concordo, desligar" : "Concordo, ligar"}
          </button>{" "}
          <button
            className="btn"
            type="button"
            onClick={() => setConfirmando(false)}
            disabled={ocupado}
          >
            Cancelar
          </button>
        </div>
      )}
      {falha && (
        <p className="error-text" role="alert">
          {falha}
        </p>
      )}
    </div>
  );
}

function Descadastrar({
  api,
  projeto,
  aoDescadastrar,
}: {
  api: ApiConfiguracoes;
  projeto: ProjetoConfig;
  aoDescadastrar: () => void;
}) {
  const [confirmando, setConfirmando] = useState(false);
  const [confirmado, setConfirmado] = useState(false);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  const descadastrar = () => {
    setOcupado(true);
    setFalha(null);
    api
      .descadastrar(projeto.id)
      .then(() => aoDescadastrar())
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setOcupado(false);
      });
  };

  return (
    <div className="descadastrar">
      {!confirmando ? (
        <button type="button" className="link perigo" onClick={() => setConfirmando(true)}>
          Descadastrar o projeto
        </button>
      ) : (
        <div
          className="confirmar-alteracao"
          role="region"
          aria-label={`Confirmar o descadastro de ${projeto.nome}`}
        >
          <p>
            O app vai desfazer o ambiente do Claude deste projeto (servidor MCP, proteção contra
            edição e, se for o último projeto, os hooks) e apagar o cadastro, o inventário e o mapa.
            Os documentos do projeto na base de conhecimento ficam, e as pastas do código não são
            tocadas.
          </p>
          <label>
            <input
              type="checkbox"
              checked={confirmado}
              onChange={(e) => setConfirmado(e.target.checked)}
            />{" "}
            Confirmo o descadastro de {projeto.nome}
          </label>
          <div className="acoes">
            <button
              className="btn"
              type="button"
              onClick={() => setConfirmando(false)}
              disabled={ocupado}
            >
              Cancelar
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={descadastrar}
              disabled={!confirmado || ocupado}
            >
              Descadastrar
            </button>
          </div>
        </div>
      )}
      {falha && (
        <p className="error-text" role="alert">
          Não foi possível descadastrar: {falha}. O cadastro continua.
        </p>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Textos editáveis

const GRUPOS: { grupo: GrupoTexto; titulo: string; descricao: string }[] = [
  {
    grupo: "template",
    titulo: "Templates dos documentos",
    descricao: "A estrutura de cada tipo de documento montado pelo app.",
  },
  {
    grupo: "roteiro",
    titulo: "Roteiros dos fluxos",
    descricao: "As instruções que o agente recebe ao iniciar cada tipo de análise.",
  },
  {
    grupo: "prompt",
    titulo: "Prompts prontos",
    descricao: "Os pedidos dos botões da tela da análise.",
  },
];

function SecaoTextos({
  api,
  textos,
  aoAbrir,
}: {
  api: ApiConfiguracoes;
  textos: TextoEditavel[];
  aoAbrir: () => void;
}) {
  return (
    <section className="secao-config" aria-label="Textos editáveis">
      <h3>Templates, roteiros e prompts</h3>
      <p className="muted">
        Ficam na base, em <code>_templates/</code>. Um arquivo na base sobrepõe o padrão do app;
        sem ele, vale o padrão. Os textos abrem no editor de texto do seu computador.
      </p>
      {GRUPOS.map((g) => (
        <div key={g.grupo} role="group" aria-label={g.titulo}>
          <h4>{g.titulo}</h4>
          <p className="muted">{g.descricao}</p>
          <table className="textos-editaveis">
            <tbody>
              {textos
                .filter((t) => t.grupo === g.grupo)
                .map((t) => (
                  <tr key={t.arquivo}>
                    <td>{t.rotulo}</td>
                    <td>
                      <span className={`estado-texto ${t.estado}`}>{ESTADO_DO_TEXTO[t.estado]}</span>
                    </td>
                    <td>
                      <AbrirTexto api={api} texto={t} aoAbrir={aoAbrir} />
                    </td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>
      ))}
    </section>
  );
}

function AbrirTexto({
  api,
  texto,
  aoAbrir,
}: {
  api: ApiConfiguracoes;
  texto: TextoEditavel;
  aoAbrir: () => void;
}) {
  const [aberto, setAberto] = useState<string | null>(null);
  const [falha, setFalha] = useState<string | null>(null);
  return (
    <>
      <button
        className="btn"
        type="button"
        aria-label={`Abrir ${texto.rotulo} no editor`}
        onClick={() => {
          setFalha(null);
          api
            .abrirTexto(texto.arquivo)
            .then((c) => {
              setAberto(c);
              aoAbrir();
            })
            .catch((e: unknown) => setFalha(mensagemDe(e)));
        }}
      >
        {texto.estado === "padrao" ? "Criar a cópia e abrir" : "Abrir no editor"}
      </button>
      {aberto && <span className="muted"> Aberto: {aberto}</span>}
      {falha && (
        <span className="error-text" role="alert">
          {" "}
          {falha}
        </span>
      )}
    </>
  );
}
