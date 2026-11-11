import { useCallback, useEffect, useState } from "react";
import { estadoClientes, type EstadoClientes, type Passo } from "./clientes";

type Carga =
  | { tipo: "verificando" }
  | { tipo: "pronto"; estado: EstadoClientes }
  | { tipo: "erro" };

/** Estado dos clientes de IA (instalado, logado) e o passo a passo do que falta. */
export function ClientesIA() {
  const [carga, setCarga] = useState<Carga>({ tipo: "verificando" });
  const [rodada, setRodada] = useState(0);

  useEffect(() => {
    let ativo = true;
    setCarga({ tipo: "verificando" });
    estadoClientes()
      .then((estado) => ativo && setCarga({ tipo: "pronto", estado }))
      .catch(() => ativo && setCarga({ tipo: "erro" }));
    return () => {
      ativo = false;
    };
  }, [rodada]);

  const verificar = useCallback(() => setRodada((r) => r + 1), []);

  return (
    <section className="clientes" aria-label="Clientes de IA">
      <h2>Clientes de IA</h2>
      {carga.tipo === "verificando" && <p className="suave">Verificando…</p>}
      {carga.tipo === "erro" && (
        <p className="suave">Não foi possível verificar os clientes de IA.</p>
      )}
      {carga.tipo === "pronto" && (
        <div className="clientes-lista">
          <ClaudeCode estado={carga.estado} />
          <ClaudeDesktop estado={carga.estado} />
        </div>
      )}
      <button type="button" onClick={verificar} disabled={carga.tipo === "verificando"}>
        Verificar de novo
      </button>
    </section>
  );
}

function ClaudeCode({ estado }: { estado: EstadoClientes }) {
  const { instalacao, login, orientacao } = estado.claudeCode;
  const instalado = instalacao.estado === "encontrado";
  let textoLogin: string;
  switch (login.estado) {
    case "logado":
      textoLogin = "Logado";
      break;
    case "naoLogado":
      textoLogin = "Sem login";
      break;
    case "indeterminado":
      textoLogin = "Login não confirmado";
      break;
    default:
      textoLogin = "";
  }
  return (
    <article className="cliente" aria-label="Claude Code">
      <h3>Claude Code (recomendado)</h3>
      <p>
        {instalado
          ? `Instalado${instalacao.versao ? `, versão ${instalacao.versao}` : ""}`
          : "Não instalado"}
      </p>
      {textoLogin && <p>{textoLogin}</p>}
      <Passos passos={orientacao} />
    </article>
  );
}

function ClaudeDesktop({ estado }: { estado: EstadoClientes }) {
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
      <Passos passos={orientacao} />
    </article>
  );
}

function Passos({ passos }: { passos: Passo[] }) {
  if (passos.length === 0) return null;
  return (
    <ol className="passos">
      {passos.map((p, i) => (
        <li key={i}>
          {p.texto}
          {p.comando && (
            <pre>
              <code>{p.comando}</code>
            </pre>
          )}
          {/* Texto para copiar: abrir o navegador a partir do app fica para o primeiro uso (#35). */}
          {p.link && (
            <>
              {" "}
              <code className="link">{p.link}</code>
            </>
          )}
        </li>
      ))}
    </ol>
  );
}
