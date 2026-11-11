import { useCallback, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { estadoClientes, type EstadoClientes, type Passo } from "./clientes";

export type CargaClientes =
  { tipo: "verificando" } | { tipo: "pronto"; estado: EstadoClientes } | { tipo: "erro" };

/** Abre um link oficial no navegador; sem o Tauri (testes, navegador), não faz nada. */
export type AbrirLink = (url: string) => Promise<void>;

const abrirPeloTauri: AbrirLink = (url) => openUrl(url).catch(() => undefined);

/**
 * Detecção dos clientes de IA com "verificar de novo". Reaproveitada pelo primeiro uso
 * e pela tela de configurações.
 */
export function useEstadoClientes(detectar: () => Promise<EstadoClientes> = estadoClientes) {
  const [carga, setCarga] = useState<CargaClientes>({ tipo: "verificando" });
  const [rodada, setRodada] = useState(0);

  useEffect(() => {
    let ativo = true;
    setCarga({ tipo: "verificando" });
    detectar()
      .then((estado) => ativo && setCarga({ tipo: "pronto", estado }))
      .catch(() => ativo && setCarga({ tipo: "erro" }));
    return () => {
      ativo = false;
    };
  }, [rodada, detectar]);

  const verificar = useCallback(() => setRodada((r) => r + 1), []);
  return { carga, verificar };
}

/** Estado dos clientes de IA (instalado, logado) e o passo a passo do que falta. */
export function ClientesIA({ abrirLink = abrirPeloTauri }: { abrirLink?: AbrirLink }) {
  const { carga, verificar } = useEstadoClientes();

  return (
    <section className="clientes" aria-label="Clientes de IA">
      <h2>Clientes de IA</h2>
      {carga.tipo === "verificando" && <p className="suave">Verificando…</p>}
      {carga.tipo === "erro" && (
        <p className="suave">Não foi possível verificar os clientes de IA.</p>
      )}
      {carga.tipo === "pronto" && (
        <div className="clientes-lista">
          <CartaoClaudeCode estado={carga.estado} abrirLink={abrirLink} />
          <CartaoClaudeDesktop estado={carga.estado} abrirLink={abrirLink} />
        </div>
      )}
      <button type="button" onClick={verificar} disabled={carga.tipo === "verificando"}>
        Verificar de novo
      </button>
    </section>
  );
}

/** Texto do login do Claude Code; vazio quando não foi verificado. */
export function textoLogin(estado: EstadoClientes): string {
  switch (estado.claudeCode.login.estado) {
    case "logado":
      return "Logado";
    case "naoLogado":
      return "Sem login";
    case "indeterminado":
      return "Login não confirmado";
    default:
      return "";
  }
}

export function CartaoClaudeCode({
  estado,
  abrirLink = abrirPeloTauri,
  titulo = "Claude Code (recomendado)",
}: {
  estado: EstadoClientes;
  abrirLink?: AbrirLink;
  titulo?: string;
}) {
  const { instalacao, orientacao } = estado.claudeCode;
  const instalado = instalacao.estado === "encontrado";
  const login = textoLogin(estado);
  return (
    <article className="cliente" aria-label="Claude Code">
      <h3>{titulo}</h3>
      <p>
        {instalado
          ? `Instalado${instalacao.versao ? `, versão ${instalacao.versao}` : ""}`
          : "Não instalado"}
      </p>
      {login && <p>{login}</p>}
      <Passos passos={orientacao} abrirLink={abrirLink} />
    </article>
  );
}

export function CartaoClaudeDesktop({
  estado,
  abrirLink = abrirPeloTauri,
}: {
  estado: EstadoClientes;
  abrirLink?: AbrirLink;
}) {
  const { instalacao, orientacao } = estado.claudeDesktop;
  if (instalacao.estado === "naoSuportado") {
    return (
      <article className="cliente" aria-label="Claude Desktop">
        <h3>Claude Desktop</h3>
        <p>Não disponível neste sistema (só macOS e Windows).</p>
      </article>
    );
  }
  return (
    <article className="cliente" aria-label="Claude Desktop">
      <h3>Claude Desktop</h3>
      <p>{instalacao.estado === "encontrado" ? "Instalado" : "Não instalado"}</p>
      <Passos passos={orientacao} abrirLink={abrirLink} />
    </article>
  );
}

/** Passo a passo oficial: comandos para copiar e links que abrem no navegador. */
export function Passos({
  passos,
  abrirLink = abrirPeloTauri,
}: {
  passos: Passo[];
  abrirLink?: AbrirLink;
}) {
  if (passos.length === 0) return null;
  return (
    <ol className="passos">
      {passos.map((p, i) => (
        <li key={i}>
          {p.texto}
          {p.comando && <Comando texto={p.comando} />}
          {p.link && (
            <>
              {" "}
              <a
                className="link"
                href={p.link}
                onClick={(e) => {
                  e.preventDefault();
                  void abrirLink(p.link!);
                }}
              >
                {p.link}
              </a>
            </>
          )}
        </li>
      ))}
    </ol>
  );
}

function Comando({ texto }: { texto: string }) {
  const [copiado, setCopiado] = useState(false);
  const copiar = () => {
    navigator.clipboard
      ?.writeText(texto)
      .then(() => setCopiado(true))
      .catch(() => undefined);
  };
  return (
    <div className="comando">
      <pre>
        <code>{texto}</code>
      </pre>
      <button type="button" onClick={copiar} aria-label={`Copiar o comando ${texto}`}>
        {copiado ? "Copiado" : "Copiar"}
      </button>
    </div>
  );
}
