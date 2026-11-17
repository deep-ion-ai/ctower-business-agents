import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { contar } from "./formatacao";
import { mensagemDe } from "./analise";
import type {
  ApiConfiguracoes,
  AvaliacaoTroca,
  Configuracoes,
  InfoBase,
  ModoTroca,
  TrocaBase,
} from "./configuracoes";
import { EscolhaBase } from "./EscolhaBase";
import { tamanhoLegivel } from "./primeiroUso";

const FOCAVEIS =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** “hoje, 09:12” ou “12/11/2026, 09:12”, no fuso do computador. */
export function quandoLegivel(segundos: number, agora = new Date()): string {
  const d = new Date(segundos * 1000);
  const hora = d.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
  return d.toDateString() === agora.toDateString()
    ? `hoje, ${hora}`
    : `${d.toLocaleDateString("pt-BR")}, ${hora}`;
}

function resumoDaBase(info: InfoBase) {
  const chips: { texto: string; tom?: "done" | "attention" }[] = [];
  if (!info.existe) {
    chips.push({ texto: "A pasta não existe mais", tom: "attention" });
    return chips;
  }
  chips.push({
    texto: `${info.arquivos} ${info.arquivos === 1 ? "documento" : "documentos"} · ${tamanhoLegivel(info.bytes)}`,
    tom: "done",
  });
  chips.push({ texto: info.git ? "versionada com git" : "sem git" });
  chips.push({
    texto: info.indiceAtualizadoEm
      ? `índice atualizado ${quandoLegivel(info.indiceAtualizadoEm)}`
      : "índice ainda não montado",
  });
  return chips;
}

/**
 * Pasta da base (#191): o caminho, quantos documentos e que tamanho, se é git e quando
 * o índice foi atualizado, com “Abrir no computador” e “Trocar a pasta…”, que abre o
 * diálogo de dois passos (escolher a pasta; o que fazer com os documentos). Mantém as
 * regras de mover e de só usar a nova.
 */
export function ConfigPastaBase({
  api,
  cfg,
  aoMudar,
}: {
  api: ApiConfiguracoes;
  cfg: Configuracoes;
  aoMudar: () => void;
}) {
  const [info, setInfo] = useState<InfoBase | null>(null);
  const [trocando, setTrocando] = useState(false);
  const [feito, setFeito] = useState<TrocaBase | null>(null);
  const [aberta, setAberta] = useState(false);
  const [falha, setFalha] = useState<string | null>(null);
  const botao = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    let ativo = true;
    setInfo(null);
    if (cfg.base) {
      Promise.resolve()
        .then(() => api.infoBase())
        .then((i) => ativo && setInfo(i))
        .catch(() => ativo && setInfo(null));
    }
    return () => {
      ativo = false;
    };
  }, [api, cfg.base]);

  const abrir = () => {
    setFalha(null);
    api
      .abrirBase()
      .then(() => setAberta(true))
      .catch((e: unknown) => setFalha(mensagemDe(e)));
  };

  return (
    <section className="config-secao" aria-labelledby="config-pasta">
      <header className="config-secao-topo">
        <h2 id="config-pasta">Pasta da base de conhecimento</h2>
        <p className="muted">
          Onde ficam as regras, o glossário, as atas e as análises aprovadas de todos os projetos.
        </p>
      </header>
      <div className="config-cartao">
        {cfg.base ? (
          <code className="config-caminho">{cfg.base}</code>
        ) : (
          <>
            <p>
              <span className="muted">Ainda não escolhida.</span>
            </p>
            <p className="muted">
              Sem a pasta da base não há onde guardar os documentos: escolha uma para cadastrar
              projetos e editar os modelos.
            </p>
          </>
        )}
        {info && (
          <div className="config-linha-chips">
            {resumoDaBase(info).map((c) => (
              <span key={c.texto} className="badge" data-tone={c.tom}>
                {c.tom === "done" ? "✓ " : ""}
                {c.texto}
              </span>
            ))}
          </div>
        )}
        {feito && <ResultadoTroca feito={feito} />}
        <div className="acoes config-acoes-inicio">
          {cfg.base && (
            <button className="btn" type="button" onClick={abrir}>
              Abrir no computador
            </button>
          )}
          <button
            ref={botao}
            className={cfg.base ? "btn" : "btn btn-primary"}
            type="button"
            onClick={() => {
              setFeito(null);
              setTrocando(true);
            }}
          >
            {cfg.base ? "Trocar a pasta…" : "Escolher a pasta…"}
          </button>
        </div>
        {aberta && !falha && (
          <p className="muted" role="status">
            Pasta aberta no gerenciador de arquivos.
          </p>
        )}
        {falha && (
          <p className="error-text" role="alert">
            {falha}
          </p>
        )}
      </div>
      {trocando && (
        <DialogoTrocaBase
          api={api}
          cfg={cfg}
          aoFechar={() => setTrocando(false)}
          aoConcluir={(t) => {
            setFeito(t);
            setTrocando(false);
            aoMudar();
          }}
        />
      )}
    </section>
  );
}

/**
 * Diálogo de dois passos para trocar a pasta: 1) escolher a pasta (com as mesmas
 * recusas do primeiro uso); 2) o que fazer com os documentos, com o aviso das
 * conversas abertas. Esc fecha, o foco fica dentro e volta a quem abriu.
 */
function DialogoTrocaBase({
  api,
  cfg,
  aoFechar,
  aoConcluir,
}: {
  api: ApiConfiguracoes;
  cfg: Configuracoes;
  aoFechar: () => void;
  aoConcluir: (t: TrocaBase) => void;
}) {
  const [etapa, setEtapa] = useState<"escolher" | "confirmar">("escolher");
  const [destino, setDestino] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const caixa = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const anterior = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    caixa.current?.focus();
    return () => anterior?.focus();
  }, []);

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

  const teclas = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      if (!ocupado) aoFechar();
      return;
    }
    if (e.key !== "Tab" || !caixa.current) return;
    const itens = Array.from(caixa.current.querySelectorAll<HTMLElement>(FOCAVEIS));
    if (itens.length === 0) return;
    const primeiro = itens[0];
    const ultimo = itens[itens.length - 1];
    if (e.shiftKey && (document.activeElement === primeiro || document.activeElement === caixa.current)) {
      e.preventDefault();
      ultimo.focus();
    } else if (!e.shiftKey && document.activeElement === ultimo) {
      e.preventDefault();
      primeiro.focus();
    }
  };

  return (
    <div className="dialogo-fundo">
      <div
        ref={caixa}
        className="dialogo dialogo-troca-base"
        role="dialog"
        aria-modal="true"
        aria-labelledby="dialogo-troca-titulo"
        tabIndex={-1}
        onKeyDown={teclas}
      >
        <p className="muted dialogo-passo">Passo {etapa === "escolher" ? 1 : 2} de 2</p>
        <h3 id="dialogo-troca-titulo">
          {etapa === "escolher"
            ? cfg.base
              ? "Para qual pasta mudar a base?"
              : "Onde guardar os documentos?"
            : "O que fazer com os documentos?"}
        </h3>
        {etapa === "escolher" && (
          <>
            <p className="muted">
              Pode ser uma pasta sincronizada (OneDrive, Google Drive) ou um repositório git seu,
              mas não a pasta do código de um projeto.
            </p>
            <EscolhaBase
              api={apiEscolha}
              atual={null}
              sugerida={cfg.base ? null : cfg.baseSugerida}
              rotuloContinuar="Continuar"
              compacto
              aoConcluir={(p) => {
                setDestino(p);
                setEtapa("confirmar");
              }}
            />
            <div className="dialogo-acoes">
              <button className="btn" type="button" onClick={aoFechar}>
                Cancelar
              </button>
            </div>
          </>
        )}
        {etapa === "confirmar" && destino && (
          <ConfirmarTroca
            api={api}
            destino={destino}
            aoOcupado={setOcupado}
            aoVoltar={() => setEtapa("escolher")}
            aoCancelar={aoFechar}
            aoConcluir={aoConcluir}
          />
        )}
      </div>
    </div>
  );
}

function ConfirmarTroca({
  api,
  destino,
  aoVoltar,
  aoCancelar,
  aoConcluir,
  aoOcupado,
}: {
  api: ApiConfiguracoes;
  destino: string;
  aoVoltar: () => void;
  aoCancelar: () => void;
  aoConcluir: (t: TrocaBase) => void;
  aoOcupado: (ocupado: boolean) => void;
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
    aoOcupado(true);
    setFalha(null);
    api
      .trocarBase(destino, modo)
      .then(aoConcluir)
      .catch((e: unknown) => {
        setFalha(mensagemDe(e));
        setOcupado(false);
        aoOcupado(false);
      });
  };

  const recusa = (m: ModoTroca) =>
    m === "mover" ? avaliacao?.recusaMover : avaliacao?.recusaReapontar;

  return (
    <div className="confirmar-troca" role="region" aria-label="Confirmar a troca da base">
      {avaliacao?.atual && (
        <p className="muted">
          De <code>{avaliacao.atual}</code>
        </p>
      )}
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
                disabled={!!avaliacao.recusaMover || ocupado}
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
              {avaliacao.recusaMover && (
                <span className="attention-text">{avaliacao.recusaMover}</span>
              )}
            </label>
            <label className={modo === "reapontar" ? "opcao escolhida" : "opcao"}>
              <input
                type="radio"
                name="modo-troca"
                value="reapontar"
                checked={modo === "reapontar"}
                disabled={!!avaliacao.recusaReapontar || ocupado}
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
              Há {contar(avaliacao.sessoesAbertas, "conversa", "conversas")} com o Claude que podem estar abertas. Feche
              as conversas antes de trocar a base, para nada ser gravado na pasta antiga.
            </p>
          )}
          {modo && (
            <label>
              <input
                type="checkbox"
                checked={confirmado}
                disabled={ocupado}
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
      <div className="dialogo-acoes dialogo-acoes-troca">
        <button className="btn" type="button" onClick={aoVoltar} disabled={ocupado}>
          ← Voltar
        </button>
        <span className="espaco" />
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
