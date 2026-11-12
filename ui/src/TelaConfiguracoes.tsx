import { useCallback, useEffect, useState } from "react";
import { mensagemDe, type SituacaoProtecao } from "./analise";
import { CadastroSistema, ResumoDoInventario } from "./CadastroSistema";
import {
  ESTADO_DO_TEXTO,
  apiConfiguracoesTauri,
  type AmbienteDetalhado,
  type ApiConfiguracoes,
  type AvaliacaoTroca,
  type Configuracoes,
  type GrupoTexto,
  type ModoTroca,
  type SistemaConfig,
  type TextoEditavel,
  type TrocaBase,
} from "./configuracoes";
import { EscolhaBase } from "./EscolhaBase";
import { EscolhaCliente } from "./EscolhaCliente";
import { AmbienteDoSistema } from "./PrepararAmbiente";
import { padraoDoCaminho, tamanhoLegivel, type PreviaSistema } from "./primeiroUso";
import "./configuracoes.css";

const CLIENTE = {
  terminalEmbutido: "Claude Code no terminal do app",
  claudeDesktop: "Claude Desktop",
} as const;

/**
 * Configurações (spec, seção 10): cliente de IA preferido, pasta da base, sistemas
 * (pastas, exclusões, ambiente do Claude, proteção contra edição, regras do sistema,
 * descadastro) e os textos editáveis da base (templates, roteiros e prompts). Monta
 * os passos do primeiro uso ({@link EscolhaCliente}, {@link EscolhaBase},
 * {@link CadastroSistema}, {@link AmbienteDoSistema}) com o que só as configurações
 * fazem: mover ou reapontar a base, desfazer o ambiente e descadastrar.
 */
export function TelaConfiguracoes({
  api = apiConfiguracoesTauri,
  aoMudarSistemas,
}: {
  api?: ApiConfiguracoes;
  /** Os sistemas ou a base mudaram: as outras telas devem reler. */
  aoMudarSistemas?: () => void;
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
    aoMudarSistemas?.();
  };

  if (!cfg) {
    return (
      <section className="configuracoes" aria-label="Configurações">
        {falha ? (
          <p className="trecho-erro" role="alert">
            Não foi possível ler as configurações: {falha}
          </p>
        ) : (
          <p className="suave">Carregando as configurações…</p>
        )}
      </section>
    );
  }

  return (
    <section className="configuracoes" aria-label="Configurações">
      <h2>Configurações</h2>
      {falha && (
        <p className="trecho-erro" role="alert">
          {falha}
        </p>
      )}
      <SecaoCliente api={api} cfg={cfg} aoMudar={carregar} />
      <SecaoBase api={api} cfg={cfg} aoMudar={mudou} />
      <SecaoSistemas api={api} cfg={cfg} textos={textos} aoMudar={mudou} aoAbrirTexto={carregar} />
      {cfg.base && <SecaoTextos api={api} textos={textos} aoAbrir={carregar} />}
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
        {!cfg.cliente && <span className="suave"> (padrão)</span>}
      </p>
      {!trocando ? (
        <button type="button" onClick={() => setTrocando(true)}>
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
        {cfg.base ? <code>{cfg.base}</code> : <span className="suave">Ainda não escolhida.</span>}
      </p>
      {feito && <ResultadoTroca feito={feito} />}
      {etapa === "ver" && (
        <button
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
      {!avaliacao && !falha && <p className="suave">Conferindo a pasta…</p>}
      {avaliacao && (
        <>
          {avaliacao.atual && (
            <p className="suave">
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
              <span className="suave">
                O app copia a base inteira, confere cada arquivo e só então passa a usar a pasta
                nova e apaga a antiga. Se algo falhar, nada muda.
              </span>
              {avaliacao.recusaMover && <span className="pendencia">{avaliacao.recusaMover}</span>}
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
              <span className="suave">
                Os documentos da pasta atual ficam onde estão, intactos, mas deixam de aparecer no
                app. Útil quando a pasta nova já é uma base (outra máquina, outro clone).
              </span>
              {avaliacao.recusaReapontar && (
                <span className="pendencia">{avaliacao.recusaReapontar}</span>
              )}
            </label>
          </fieldset>
          {avaliacao.sessoesAbertas > 0 && (
            <p className="pendencia" role="status">
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
            <p className="trecho-erro" role="alert">
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
        <p className="trecho-erro" role="alert">
          {falha}
        </p>
      )}
      <div className="acoes">
        <button type="button" onClick={aoCancelar} disabled={ocupado}>
          Cancelar
        </button>
        <button
          type="button"
          className="primario"
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
    <div className="confirmacao" role="status">
      <p>
        {feito.modo === "mover"
          ? `Base movida: ${feito.copiados} arquivos copiados e conferidos${
              feito.origemApagada ? "; a pasta antiga foi apagada" : ""
            }.`
          : "O app passou a usar a pasta nova."}{" "}
        {feito.indexados !== null && `Índice reconstruído com ${feito.indexados} documentos.`}
      </p>
      {feito.avisos.map((a) => (
        <p key={a} className="pendencia">
          {a}
        </p>
      ))}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sistemas

function SecaoSistemas({
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
    <section className="secao-config" aria-label="Sistemas">
      <h3>Sistemas</h3>
      {cfg.sistemas.length === 0 && <p className="suave">Nenhum sistema cadastrado.</p>}
      {cfg.configClaude && (
        <p className="suave">
          O ambiente do Claude fica em {cfg.configClaude.estado} (servidor MCP) e{" "}
          {cfg.configClaude.settings} (hooks e proteção).
        </p>
      )}
      {cfg.sistemas.map((s) => (
        <CartaoSistema
          key={s.id}
          api={api}
          sistema={s}
          regras={textos.find((t) => t.grupo === "regrasDoSistema" && t.id === s.id)}
          aoMudar={aoMudar}
          aoAbrirTexto={aoAbrirTexto}
        />
      ))}
      {cfg.base &&
        (cadastrando ? (
          <>
            <CadastroSistema
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
          <button type="button" onClick={() => setCadastrando(true)}>
            Cadastrar outro sistema
          </button>
        ))}
    </section>
  );
}

function CartaoSistema({
  api,
  sistema,
  regras,
  aoMudar,
  aoAbrirTexto,
}: {
  api: ApiConfiguracoes;
  sistema: SistemaConfig;
  regras?: TextoEditavel;
  aoMudar: () => void;
  aoAbrirTexto: () => void;
}) {
  // Muda depois de desfazer o ambiente, para a verificação rodar de novo.
  const [vez, setVez] = useState(0);
  return (
    <article className="cartao-sistema" aria-label={sistema.nome}>
      <h4>{sistema.nome}</h4>
      <ul className="suave">
        {sistema.pastas.map((p) => (
          <li key={p.caminho}>
            {p.caminho}
            {p.prefixo && ` (aparece como ${p.prefixo}/)`}
            {!p.existe && <strong className="pendencia"> · a pasta não existe mais</strong>}
          </li>
        ))}
      </ul>
      <p className="suave">
        Para mudar as pastas, descadastre o sistema e cadastre de novo (os documentos ficam na
        base).
      </p>
      <AjusteDeExclusoes api={api} sistema={sistema} aoSalvar={aoMudar} />
      <div role="group" aria-label={`Ambiente do Claude de ${sistema.nome}`}>
        <h5>Ambiente do Claude</h5>
        <DetalheDoAmbiente key={`d${vez}`} api={api} sistema={sistema.id} />
        <AmbienteDoSistema key={vez} api={api} sistema={sistema.id} />
        <DesfazerAmbiente api={api} sistema={sistema} aoDesfazer={() => setVez((v) => v + 1)} />
      </div>
      <Protecao key={`p${vez}`} api={api} sistema={sistema} />
      {regras && (
        <div role="group" aria-label={`Regras do sistema ${sistema.nome}`}>
          <h5>Regras do sistema</h5>
          <p className="suave">
            Instruções que o agente recebe em toda análise deste sistema ({regras.arquivo}):{" "}
            {ESTADO_DO_TEXTO[regras.estado]}.
          </p>
          <AbrirTexto api={api} texto={regras} aoAbrir={aoAbrirTexto} />
        </div>
      )}
      <Descadastrar api={api} sistema={sistema} aoDescadastrar={aoMudar} />
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
  sistema,
  aoSalvar,
}: {
  api: ApiConfiguracoes;
  sistema: SistemaConfig;
  aoSalvar: () => void;
}) {
  const [exclusoes, setExclusoes] = useState(sistema.exclusoes.join("\n"));
  const [inclusoes, setInclusoes] = useState<string[]>(sistema.inclusoes);
  const [previa, setPrevia] = useState<PreviaSistema | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const [salvo, setSalvo] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  const ajuste = (inc = inclusoes) => ({
    sistema: sistema.id,
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
        <p className="confirmacao" role="status">
          Exclusões salvas e sistema inventariado de novo.
        </p>
      )}
      {falha && (
        <p className="trecho-erro" role="alert">
          {falha}
        </p>
      )}
      <div className="acoes">
        <button type="button" onClick={() => verPrevia()} disabled={ocupado}>
          Ver o que será analisado
        </button>
        <button type="button" className="primario" onClick={salvar} disabled={ocupado}>
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

function DetalheDoAmbiente({ api, sistema }: { api: ApiConfiguracoes; sistema: string }) {
  const [ambiente, setAmbiente] = useState<AmbienteDetalhado | null>(null);
  useEffect(() => {
    let ativo = true;
    api
      .verificarAmbiente(sistema)
      .then((a) => ativo && setAmbiente(a))
      .catch(() => ativo && setAmbiente(null));
    return () => {
      ativo = false;
    };
  }, [api, sistema]);
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
  sistema,
  aoDesfazer,
}: {
  api: ApiConfiguracoes;
  sistema: SistemaConfig;
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
      .desfazerAmbiente(sistema.id)
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
        <div className="confirmacao" role="status">
          <p>
            Ambiente desfeito: a configuração do Claude voltou a ser a de antes para este sistema.
          </p>
          {feito.map((a) => (
            <p key={a} className="pendencia">
              {a}
            </p>
          ))}
        </div>
      )}
      {!confirmando ? (
        <button
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
            O app vai tirar da configuração do Claude o servidor MCP deste sistema, a proteção
            contra edição (se ligada) e, se nenhum outro sistema usar, os hooks. Sem eles, as
            conversas nesta pasta não enxergam o ctower. Você pode preparar de novo depois.
          </p>
          <button type="button" className="primario" onClick={desfazer} disabled={ocupado}>
            Desfazer
          </button>{" "}
          <button type="button" onClick={() => setConfirmando(false)} disabled={ocupado}>
            Cancelar
          </button>
        </div>
      )}
      {falha && (
        <p className="trecho-erro" role="alert">
          {falha}
        </p>
      )}
    </div>
  );
}

function Protecao({ api, sistema }: { api: ApiConfiguracoes; sistema: SistemaConfig }) {
  const [situacao, setSituacao] = useState<SituacaoProtecao | null>(null);
  const [confirmando, setConfirmando] = useState(false);
  const [ocupado, setOcupado] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);

  useEffect(() => {
    let ativo = true;
    api
      .situacaoProtecao(sistema.id)
      .then((s) => ativo && setSituacao(s))
      .catch((e: unknown) => ativo && setFalha(mensagemDe(e)));
    return () => {
      ativo = false;
    };
  }, [api, sistema.id]);

  const mudar = (ligada: boolean) => {
    setOcupado(true);
    setFalha(null);
    api
      .mudarProtecao(sistema.id, ligada)
      .then((s) => {
        setSituacao(s);
        setConfirmando(false);
      })
      .catch((e: unknown) => setFalha(mensagemDe(e)))
      .finally(() => setOcupado(false));
  };

  return (
    <div role="group" aria-label={`Proteção contra edição de ${sistema.nome}`}>
      <h5>Proteção contra edição</h5>
      {situacao && (
        <p>
          {situacao.ligada
            ? situacao.completa
              ? "Ligada: o Claude não edita arquivos nem usa o terminal nas pastas do sistema."
              : "Ligada, mas faltam regras no arquivo de configuração do Claude. Ligue de novo para completar."
            : "Desligada (padrão): o app só avisa quando a pasta do sistema é alterada."}
        </p>
      )}
      {situacao && !confirmando && (
        <button type="button" onClick={() => setConfirmando(true)}>
          {situacao.ligada ? "Desligar a proteção" : "Ligar a proteção"}
        </button>
      )}
      {situacao && confirmando && (
        <div className="confirmar-alteracao">
          {situacao.ligada ? (
            <p>
              O app vai tirar de {situacao.arquivo} só as regras que ele mesmo acrescentou, e o
              terminal (Bash) volta a funcionar nas conversas deste sistema.
            </p>
          ) : (
            <>
              <p>
                O app vai acrescentar em {situacao.arquivo} estas regras, que valem mesmo que você
                aprove a edição, e negar o terminal ({situacao.negadasPeloHook.join(", ")}) nas
                conversas deste sistema:
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
            className="primario"
            onClick={() => mudar(!situacao.ligada)}
            disabled={ocupado}
          >
            {situacao.ligada ? "Concordo, desligar" : "Concordo, ligar"}
          </button>{" "}
          <button type="button" onClick={() => setConfirmando(false)} disabled={ocupado}>
            Cancelar
          </button>
        </div>
      )}
      {falha && (
        <p className="trecho-erro" role="alert">
          {falha}
        </p>
      )}
    </div>
  );
}

function Descadastrar({
  api,
  sistema,
  aoDescadastrar,
}: {
  api: ApiConfiguracoes;
  sistema: SistemaConfig;
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
      .descadastrar(sistema.id)
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
          Descadastrar o sistema
        </button>
      ) : (
        <div
          className="confirmar-alteracao"
          role="region"
          aria-label={`Confirmar o descadastro de ${sistema.nome}`}
        >
          <p>
            O app vai desfazer o ambiente do Claude deste sistema (servidor MCP, proteção contra
            edição e, se for o último sistema, os hooks) e apagar o cadastro, o inventário e o mapa.
            Os documentos do sistema na base de conhecimento ficam, e as pastas do código não são
            tocadas.
          </p>
          <label>
            <input
              type="checkbox"
              checked={confirmado}
              onChange={(e) => setConfirmado(e.target.checked)}
            />{" "}
            Confirmo o descadastro de {sistema.nome}
          </label>
          <div className="acoes">
            <button type="button" onClick={() => setConfirmando(false)} disabled={ocupado}>
              Cancelar
            </button>
            <button
              type="button"
              className="primario"
              onClick={descadastrar}
              disabled={!confirmado || ocupado}
            >
              Descadastrar
            </button>
          </div>
        </div>
      )}
      {falha && (
        <p className="trecho-erro" role="alert">
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
      <p className="suave">
        Ficam na base, em <code>_templates/</code>. Um arquivo na base sobrepõe o padrão do app;
        sem ele, vale o padrão. Os textos abrem no editor de texto do seu computador.
      </p>
      {GRUPOS.map((g) => (
        <div key={g.grupo} role="group" aria-label={g.titulo}>
          <h4>{g.titulo}</h4>
          <p className="suave">{g.descricao}</p>
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
      {aberto && <span className="suave"> Aberto: {aberto}</span>}
      {falha && (
        <span className="trecho-erro" role="alert">
          {" "}
          {falha}
        </span>
      )}
    </>
  );
}
