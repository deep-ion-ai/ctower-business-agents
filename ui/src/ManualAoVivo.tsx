import { useEffect, useState, type ClipboardEvent, type DragEvent, type FormEvent } from "react";
import type { Revisao } from "./AcoesRevisao";
import { mensagemDe, type PassoManual } from "./analise";
import { Alerta, EstadoVazio, Selo, TextoRico, TituloSecao } from "./componentes";
import {
  AVISO_DADOS_SENSIVEIS,
  apiManualTauri,
  comoPng,
  type ApiManual,
} from "./manual";

/** Imagem de um print da base, lida pelo app (o arquivo não é servido à tela). */
export function ImagemDaBase({
  projeto,
  arquivo,
  alt,
  api = apiManualTauri,
}: {
  projeto: string;
  /** PNG relativo à raiz do projeto na base. */
  arquivo: string;
  alt: string;
  api?: ApiManual;
}) {
  const [url, setUrl] = useState<string | null>(null);
  const [falhou, setFalhou] = useState(false);
  useEffect(() => {
    let vivo = true;
    let criada: string | null = null;
    setFalhou(false);
    api
      .lerPrint(projeto, arquivo)
      .then((bytes) => {
        if (!vivo) return;
        criada = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
        setUrl(criada);
      })
      .catch(() => vivo && setFalhou(true));
    return () => {
      vivo = false;
      if (criada) URL.revokeObjectURL(criada);
    };
  }, [api, projeto, arquivo]);
  if (falhou) {
    return (
      <span className="muted" role="img" aria-label={alt}>
        Imagem indisponível: {alt}
      </span>
    );
  }
  if (!url) return <span className="muted">Carregando a imagem…</span>;
  return <img className="print-passo" src={url} alt={alt} />;
}

/** Print escolhido, ainda não enviado. */
interface Escolhido {
  png: Uint8Array;
  url: string;
}

function FormPrint({
  passo,
  ocupado,
  aoSalvar,
  aoCancelar,
}: {
  passo: PassoManual;
  ocupado: boolean;
  aoSalvar: (png: Uint8Array, legenda: string, alt: string) => Promise<void>;
  aoCancelar: () => void;
}) {
  const [escolhido, setEscolhido] = useState<Escolhido | null>(null);
  const [legenda, setLegenda] = useState(
    passo.print?.legenda ?? (passo.tela ? `Tela ${passo.tela}` : passo.titulo),
  );
  const [alt, setAlt] = useState(
    passo.print?.texto_alternativo ?? `Print do passo ${passo.numero}: ${passo.titulo}`,
  );
  const [erro, setErro] = useState<string | null>(null);
  useEffect(() => () => {
    if (escolhido) URL.revokeObjectURL(escolhido.url);
  }, [escolhido]);

  const receber = (arquivo: Blob | null | undefined) => {
    if (!arquivo) return;
    setErro(null);
    comoPng(arquivo)
      .then((png) =>
        setEscolhido({ png, url: URL.createObjectURL(new Blob([png], { type: "image/png" })) }),
      )
      .catch((e: unknown) => setErro(mensagemDe(e)));
  };
  const colar = (e: ClipboardEvent<HTMLDivElement>) => {
    const item = Array.from(e.clipboardData?.items ?? []).find((i) => i.type.startsWith("image/"));
    if (item) {
      e.preventDefault();
      receber(item.getAsFile());
    }
  };
  const soltar = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    receber(e.dataTransfer?.files?.[0]);
  };
  const valido = !!escolhido && legenda.trim() !== "" && alt.trim() !== "";
  const salvar = (e: FormEvent) => {
    e.preventDefault();
    if (!escolhido || !valido) return;
    aoSalvar(escolhido.png, legenda.trim(), alt.trim()).catch((x: unknown) =>
      setErro(mensagemDe(x)),
    );
  };

  return (
    <form className="form-print" aria-label={`Print do passo ${passo.numero}`} onSubmit={salvar}>
      <Alerta tom="atencao" titulo="Dados sensíveis." role="note">
        {AVISO_DADOS_SENSIVEIS}
      </Alerta>
      <div
        className="area-print"
        tabIndex={0}
        role="group"
        aria-label="Cole (Ctrl+V), arraste ou escolha a imagem"
        onPaste={colar}
        onDragOver={(e) => e.preventDefault()}
        onDrop={soltar}
      >
        {escolhido ? (
          <img className="print-passo" src={escolhido.url} alt="Prévia do print" />
        ) : (
          <span className="muted">Cole a imagem aqui (Ctrl+V) ou arraste o arquivo.</span>
        )}
        <label className="btn">
          Escolher arquivo
          <input
            type="file"
            accept="image/png,image/jpeg,image/webp,image/gif"
            hidden
            aria-label="Arquivo da imagem"
            onChange={(e) => receber(e.target.files?.[0])}
          />
        </label>
      </div>
      <label>
        Legenda
        <input value={legenda} onChange={(e) => setLegenda(e.target.value)} />
      </label>
      <label>
        Texto alternativo
        <input value={alt} onChange={(e) => setAlt(e.target.value)} />
      </label>
      {erro && (
        <p className="error-text" role="alert">
          {erro}
        </p>
      )}
      <div className="acoes">
        <button type="submit" className="btn btn-primary" disabled={ocupado || !valido}>
          Salvar o print
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

function FormPasso({
  passo,
  ocupado,
  aoSalvar,
  aoCancelar,
}: {
  passo: PassoManual;
  ocupado: boolean;
  aoSalvar: (p: { titulo: string; instrucao: string; tela: string; resultado: string }) => void;
  aoCancelar: () => void;
}) {
  const [titulo, setTitulo] = useState(passo.titulo);
  const [instrucao, setInstrucao] = useState(passo.instrucao);
  const [tela, setTela] = useState(passo.tela ?? "");
  const [resultado, setResultado] = useState(passo.resultado ?? "");
  const valido = titulo.trim() !== "" && instrucao.trim() !== "";
  return (
    <form
      className="form-passo"
      aria-label={`Editar o passo ${passo.numero}`}
      onSubmit={(e) => {
        e.preventDefault();
        if (valido) aoSalvar({ titulo, instrucao, tela, resultado });
      }}
    >
      <label>
        Título
        <input value={titulo} onChange={(e) => setTitulo(e.target.value)} />
      </label>
      <label>
        Instrução
        <textarea rows={3} value={instrucao} onChange={(e) => setInstrucao(e.target.value)} />
      </label>
      <label>
        Tela (print sugerido)
        <input value={tela} onChange={(e) => setTela(e.target.value)} />
      </label>
      <label>
        Resultado
        <textarea rows={2} value={resultado} onChange={(e) => setResultado(e.target.value)} />
      </label>
      <div className="acoes">
        <button type="submit" className="btn btn-primary" disabled={ocupado || !valido}>
          Salvar
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

function FormLegenda({
  passo,
  ocupado,
  aoSalvar,
  aoCancelar,
}: {
  passo: PassoManual;
  ocupado: boolean;
  aoSalvar: (legenda: string, alt: string) => void;
  aoCancelar: () => void;
}) {
  const [legenda, setLegenda] = useState(passo.print?.legenda ?? "");
  const [alt, setAlt] = useState(passo.print?.texto_alternativo ?? "");
  const valido = legenda.trim() !== "" && alt.trim() !== "";
  return (
    <form
      className="form-passo"
      aria-label={`Legenda do print do passo ${passo.numero}`}
      onSubmit={(e) => {
        e.preventDefault();
        if (valido) aoSalvar(legenda.trim(), alt.trim());
      }}
    >
      <label>
        Legenda
        <input value={legenda} onChange={(e) => setLegenda(e.target.value)} />
      </label>
      <label>
        Texto alternativo
        <input value={alt} onChange={(e) => setAlt(e.target.value)} />
      </label>
      <div className="acoes">
        <button type="submit" className="btn btn-primary" disabled={ocupado || !valido}>
          Salvar
        </button>
        <button type="button" className="btn" onClick={aoCancelar}>
          Cancelar
        </button>
      </div>
    </form>
  );
}

type Form = "passo" | "print" | "legenda" | null;

function CartaoPasso({
  projeto,
  analise,
  passo,
  revisao,
  api,
}: {
  projeto: string;
  analise: string;
  passo: PassoManual;
  revisao?: Revisao;
  api: ApiManual;
}) {
  const [form, setForm] = useState<Form>(null);
  const [enviando, setEnviando] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const p = passo;
  const ocupado = (revisao?.ocupado ?? true) || enviando;
  const pendente = !p.print && !p.print_dispensado;
  const acao = (f: () => Promise<void>) => {
    setEnviando(true);
    setErro(null);
    return f()
      .then(() => setForm(null))
      .catch((e: unknown) => {
        setErro(mensagemDe(e));
        throw e;
      })
      .finally(() => setEnviando(false));
  };
  const decidir = (d: Parameters<Revisao["aoDecidir"]>[0]) =>
    revisao && acao(() => revisao.aoDecidir(d)).catch(() => undefined);

  return (
    <li className="passo-manual" aria-label={`Passo ${p.numero}`} data-pendente={pendente || undefined}>
      <div className="linha-cabecalho">
        <strong>
          Passo {p.numero} · {p.titulo}
        </strong>
        {pendente && <Selo tom="atencao">sem print</Selo>}
        {p.print_dispensado && <Selo tom="neutro">print dispensado</Selo>}
      </div>
      {form === "passo" && revisao ? (
        <FormPasso
          passo={p}
          ocupado={ocupado}
          aoCancelar={() => setForm(null)}
          aoSalvar={(x) =>
            decidir({
              tipo: "editar-passo",
              numero: p.numero,
              titulo: x.titulo,
              instrucao: x.instrucao,
              tela: x.tela.trim() || null,
              resultado: x.resultado.trim() || null,
            })
          }
        />
      ) : (
        <TextoRico texto={p.instrucao} />
      )}
      {p.print ? (
        <figure className="figura-passo">
          <ImagemDaBase
            projeto={projeto}
            arquivo={p.print.arquivo}
            alt={p.print.texto_alternativo}
            api={api}
          />
          <figcaption>
            Figura {p.numero}. {p.print.legenda}
          </figcaption>
        </figure>
      ) : (
        !p.print_dispensado && (
          <p className="print-sugerido">
            <strong>Print sugerido:</strong> {p.tela ? `tela ${p.tela}` : "a tela deste passo"}
          </p>
        )
      )}
      {p.resultado && (
        <p>
          <strong>Resultado:</strong> {p.resultado}
        </p>
      )}
      {form === "print" && revisao && (
        <FormPrint
          passo={p}
          ocupado={ocupado}
          aoCancelar={() => setForm(null)}
          aoSalvar={(png, legenda, alt) =>
            acao(() => api.anexarPrint(projeto, analise, p.numero, png, legenda, alt))
          }
        />
      )}
      {form === "legenda" && revisao && (
        <FormLegenda
          passo={p}
          ocupado={ocupado}
          aoCancelar={() => setForm(null)}
          aoSalvar={(legenda, alt) =>
            decidir({ tipo: "legenda-print", numero: p.numero, legenda, texto_alternativo: alt })
          }
        />
      )}
      {erro && (
        <p className="error-text" role="alert">
          {erro}
        </p>
      )}
      {revisao && form === null && (
        <div className="acoes-revisao" role="group" aria-label={`Revisão do passo ${p.numero}`}>
          <button type="button" className="btn" disabled={ocupado} onClick={() => setForm("print")}>
            {p.print ? "Trocar o print" : "Incluir print"}
          </button>
          {p.print && (
            <>
              <button
                type="button"
                className="btn"
                disabled={ocupado}
                onClick={() => setForm("legenda")}
              >
                Editar legenda
              </button>
              <button
                type="button"
                className="btn"
                disabled={ocupado}
                onClick={() =>
                  acao(() => api.removerPrint(projeto, analise, p.numero)).catch(() => undefined)
                }
              >
                Remover o print
              </button>
            </>
          )}
          {!p.print && (
            <button
              type="button"
              className="btn"
              disabled={ocupado}
              onClick={() =>
                decidir({
                  tipo: "dispensar-print",
                  numero: p.numero,
                  dispensado: !p.print_dispensado,
                })
              }
            >
              {p.print_dispensado ? "Pedir o print" : "Dispensar o print"}
            </button>
          )}
          <button type="button" className="btn" disabled={ocupado} onClick={() => setForm("passo")}>
            Editar o passo
          </button>
          <button
            type="button"
            className="btn"
            disabled={ocupado}
            onClick={() =>
              acao(() => api.removerPasso(projeto, analise, p.numero)).catch(() => undefined)
            }
          >
            Remover o passo
          </button>
        </div>
      )}
    </li>
  );
}

/**
 * Passo a passo do manual do usuário (#272): cada passo com a instrução, o print
 * (ou o marcador "print sugerido") e o resultado. Na revisão, o analista inclui,
 * troca e remove o print, edita a legenda, dispensa o print, edita e remove o
 * passo. Sem `revisao` (análise aprovada), só leitura.
 */
export function ManualAoVivo({
  projeto,
  analise,
  passos,
  revisao,
  api = apiManualTauri,
}: {
  projeto: string;
  analise: string;
  passos: PassoManual[];
  revisao?: Revisao;
  api?: ApiManual;
}) {
  const semPrint = passos.filter((p) => !p.print && !p.print_dispensado).length;
  return (
    <section aria-label="Passo a passo" className="manual-ao-vivo">
      <TituloSecao
        nivel={3}
        acoes={
          semPrint > 0 && (
            <Selo tom="atencao">
              {semPrint === 1 ? "1 passo sem print" : `${semPrint} passos sem print`}
            </Selo>
          )
        }
      >
        Passo a passo
      </TituloSecao>
      {passos.length === 0 ? (
        <EstadoVazio>Nenhum passo registrado ainda.</EstadoVazio>
      ) : (
        <ol className="passos-manual">
          {passos.map((p) => (
            <CartaoPasso
              key={`${p.numero}-${p.titulo}`}
              projeto={projeto}
              analise={analise}
              passo={p}
              revisao={revisao}
              api={api}
            />
          ))}
        </ol>
      )}
    </section>
  );
}
